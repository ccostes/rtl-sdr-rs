use rtl_sdr_rs::{DirectSampleMode, DeviceId, RtlSdr, TunerGain, DEFAULT_BUF_LENGTH};
use std::cmp;
use std::env;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const DEFAULT_PORT: &str = "1234";
const DEFAULT_SAMPLE_RATE: u32 = 2_048_000;
const DEFAULT_FREQUENCY: u32 = 100_000_000;
const DEFAULT_QUEUE_LIMIT: usize = 500;
const ACCEPT_POLL_INTERVAL_MS: u64 = 100;

#[derive(Clone, Debug)]
struct AppConfig {
	address: String,
	port: u16,
	frequency: u32,
	sample_rate: u32,
	buffer_count: Option<u32>,
	queue_limit: usize,
	device_index: usize,
	ppm_error: i32,
	gain: Option<i32>,
	enable_bias_tee: bool,
	direct_sampling: bool,
}

#[derive(Debug)]
enum ControlMessage {
	SetFrequency(u32),
	SetSampleRate(u32),
	SetGainMode(bool),
	SetGain(i32),
	SetFreqCorrection(i32),
	SetIfGain { stage: u16, gain: i16 },
	SetTestMode(bool),
	SetAgcMode(bool),
	SetDirectSampling(u32),
	SetOffsetTuning(bool),
	SetRtlXtal(u32),
	SetTunerXtal(u32),
	SetGainByIndex(u32),
	SetBiasTee(bool),
	Shutdown,
}

struct ServeOutcome {
	sdr: RtlSdr,
	error: Option<String>,
}

fn main() {
    stderrlog::new().verbosity(log::Level::Info).init().unwrap();
	if let Err(err) = run() {
		eprintln!("rtl_tcp: {}", err);
		std::process::exit(1);
	}
}

fn run() -> Result<(), String> {
	let config = parse_args()?;

	let shutdown = Arc::new(AtomicBool::new(false));
	{
		let shutdown_flag = shutdown.clone();
		ctrlc::set_handler(move || {
			shutdown_flag.store(true, Ordering::SeqCst);
		})
		.map_err(|e| format!("Failed to set signal handler: {}", e))?;
	}

	let mut sdr = setup_device(&config)?;

	let listen_addr: SocketAddr = format!("{}:{}", config.address, config.port)
		.parse()
		.map_err(|e| format!("Invalid listen address: {}", e))?;

	let listener = TcpListener::bind(listen_addr)
		.map_err(|e| format!("Failed to bind socket: {}", e))?;
	listener
		.set_nonblocking(true)
		.map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;

	println!("Listening on {}", listen_addr);

	loop {
		if shutdown.load(Ordering::Relaxed) {
			break;
		}

		match listener.accept() {
			Ok((stream, addr)) => {
				println!("Client accepted from {}", addr);
				let outcome = serve_client(sdr, stream, addr, &config, shutdown.clone());
				sdr = outcome.sdr;
				if let Some(err) = outcome.error {
					eprintln!("Connection ended: {}", err);
				} else {
					println!("Connection closed");
				}
				if shutdown.load(Ordering::Relaxed) {
					break;
				}
			}
			Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
				thread::sleep(Duration::from_millis(ACCEPT_POLL_INTERVAL_MS));
			}
			Err(err) => {
				return Err(format!("Accept failed: {}", err));
			}
		}
	}

	sdr.close()
		.map_err(|e| format!("Failed to close device: {}", e))?;
	println!("bye!");
	Ok(())
}

fn parse_args() -> Result<AppConfig, String> {
	let mut config = AppConfig {
		address: "127.0.0.1".to_string(),
		port: DEFAULT_PORT.parse::<u16>().unwrap(),
		frequency: DEFAULT_FREQUENCY,
		sample_rate: DEFAULT_SAMPLE_RATE,
		buffer_count: None,
		queue_limit: DEFAULT_QUEUE_LIMIT,
		device_index: 0,
		ppm_error: 0,
		gain: None,
		enable_bias_tee: false,
		direct_sampling: false,
	};

	let args: Vec<String> = env::args().skip(1).collect();
	let mut idx = 0;
	while idx < args.len() {
		match args[idx].as_str() {
			"-h" | "--help" => {
				print_usage();
				std::process::exit(0);
			}
			"-a" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -a")?;
				config.address = value.clone();
			}
			"-p" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -p")?;
				config.port = value
					.parse::<u16>()
					.map_err(|e| format!("Invalid port: {}", e))?;
			}
			"-f" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -f")?;
				config.frequency = parse_scaled(value)?;
			}
			"-g" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -g")?;
				let gain = value
					.parse::<f32>()
					.map_err(|e| format!("Invalid gain: {}", e))?;
				config.gain = Some((gain * 10.0).round() as i32);
			}
			"-s" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -s")?;
				config.sample_rate = parse_scaled(value)?;
			}
			"-b" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -b")?;
				config.buffer_count = Some(
					value
						.parse::<u32>()
						.map_err(|e| format!("Invalid buffer count: {}", e))?,
				);
			}
			"-n" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -n")?;
				config.queue_limit = value
					.parse::<usize>()
					.map_err(|e| format!("Invalid queue limit: {}", e))?;
			}
			"-d" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -d")?;
				config.device_index = value
					.parse::<usize>()
					.map_err(|e| format!("Invalid device index: {}", e))?;
			}
			"-P" => {
				idx += 1;
				let value = args.get(idx).ok_or("Missing value for -P")?;
				config.ppm_error = value
					.parse::<i32>()
					.map_err(|e| format!("Invalid ppm value: {}", e))?;
			}
			"-T" => {
				config.enable_bias_tee = true;
			}
			"-D" => {
				config.direct_sampling = true;
			}
			other => {
				return Err(format!("Unknown argument: {}", other));
			}
		}
		idx += 1;
	}

	Ok(config)
}

fn print_usage() {
	println!("rtl_tcp, an I/Q spectrum server for RTL-SDR receivers");
	println!("Usage: rtl_tcp [options]\n");
	println!("  -a listen address (default: 127.0.0.1)");
	println!("  -p listen port (default: {})", DEFAULT_PORT);
	println!("  -f frequency to tune to [Hz]");
	println!("  -g gain (default: auto)");
	println!("  -s samplerate in Hz (default: {} Hz)", DEFAULT_SAMPLE_RATE);
	println!("  -b number of buffers (unused, compatibility only)");
	println!("  -n max number of buffered blocks (default: {})", DEFAULT_QUEUE_LIMIT);
	println!("  -d device index (default: 0)");
	println!("  -P ppm error (default: 0)");
	println!("  -T enable bias-T on GPIO PIN 0");
	println!("  -D enable direct sampling");
}

fn parse_scaled(value: &str) -> Result<u32, String> {
	if value.is_empty() {
		return Err("Empty numeric value".to_string());
	}
	let mut factor = 1f64;
	let mut digits = value;
	if let Some(last) = value.chars().last() {
		match last {
			'k' | 'K' => {
				factor = 1e3;
				digits = &value[..value.len() - 1];
			}
			'M' | 'm' => {
				factor = 1e6;
				digits = &value[..value.len() - 1];
			}
			'G' | 'g' => {
				factor = 1e9;
				digits = &value[..value.len() - 1];
			}
			_ => {}
		}
	}
	let number = digits
		.parse::<f64>()
		.map_err(|e| format!("Invalid number '{}': {}", value, e))?;
	if number < 0.0 {
		return Err(format!("Value must be positive: {}", value));
	}
	let hz = number * factor;
	if hz > u32::MAX as f64 {
		return Err(format!("Value too large: {}", value));
	}
	Ok(hz.round() as u32)
}

fn setup_device(config: &AppConfig) -> Result<RtlSdr, String> {
	let mut sdr = RtlSdr::open(DeviceId::Index(config.device_index))
		.map_err(|e| format!("Failed to open device: {}", e))?;

	if config.direct_sampling {
		sdr
			.set_direct_sampling(DirectSampleMode::OnSwap)
			.map_err(|e| format!("Failed to enable direct sampling: {}", e))?;
	}

	if config.ppm_error != 0 {
		sdr
			.set_freq_correction(config.ppm_error)
			.map_err(|e| format!("Failed to set PPM: {}", e))?;
	}

	sdr
		.set_sample_rate(config.sample_rate)
		.map_err(|e| format!("Failed to set sample rate: {}", e))?;

	sdr
		.set_center_freq(config.frequency)
		.map_err(|e| format!("Failed to set frequency: {}", e))?;

	match config.gain {
		None => {
			sdr
				.set_tuner_gain(TunerGain::Auto)
				.map_err(|e| format!("Failed to enable auto gain: {}", e))?;
		}
		Some(gain) => {
			sdr
				.set_tuner_gain(TunerGain::Manual(gain))
				.map_err(|e| format!("Failed to set tuner gain: {}", e))?;
		}
	}

	sdr
		.set_bias_tee(config.enable_bias_tee)
		.map_err(|e| format!("Failed to set bias tee: {}", e))?;

	sdr
		.reset_buffer()
		.map_err(|e| format!("Failed to reset buffers: {}", e))?;

	println!("Tuned to {} Hz", config.frequency);
	println!("Sampling at {} S/s", config.sample_rate);

	Ok(sdr)
}

fn serve_client(
	mut sdr: RtlSdr,
	mut stream: TcpStream,
	addr: SocketAddr,
	config: &AppConfig,
	global_shutdown: Arc<AtomicBool>,
) -> ServeOutcome {
	println!("Handshake with {}", addr);
	let queue_capacity = cmp::max(1, config.queue_limit);
	let connection_stop = Arc::new(AtomicBool::new(false));
	let mut outcome_error: Option<String> = None;

	let mut gain_values = match sdr.get_tuner_gains() {
		Ok(gains) => gains,
		Err(err) => {
			outcome_error = Some(format!("Failed to query tuner gains: {}", err));
			return ServeOutcome { sdr, error: outcome_error };
		}
	};
	let tuner_type = detect_tuner_type(&sdr);
	if let Err(err) = send_handshake(&mut stream, tuner_type, gain_values.len() as u32) {
		outcome_error = Some(format!("Failed to send handshake: {}", err));
		return ServeOutcome { sdr, error: outcome_error };
	}

	let (data_tx, data_rx) = mpsc::sync_channel::<Vec<u8>>(queue_capacity);
	let (ctrl_tx, ctrl_rx) = mpsc::channel::<ControlMessage>();

	let send_stream = match stream.try_clone() {
		Ok(clone) => clone,
		Err(err) => {
			outcome_error = Some(format!("Failed to clone stream for sender: {}", err));
			return ServeOutcome { sdr, error: outcome_error };
		}
	};
	let sender_stop = connection_stop.clone();
	let sender_shutdown = global_shutdown.clone();
	let sender_handle = thread::spawn(move || sender_loop(send_stream, data_rx, sender_stop, sender_shutdown));

	let command_stream = match stream.try_clone() {
		Ok(clone) => clone,
		Err(err) => {
			outcome_error = Some(format!("Failed to clone stream for commands: {}", err));
			connection_stop.store(true, Ordering::SeqCst);
			drop(data_tx);
			let _ = sender_handle.join();
			return ServeOutcome { sdr, error: outcome_error };
		}
	};
	let cmd_stop = connection_stop.clone();
	let cmd_shutdown = global_shutdown.clone();
	let ctrl_tx_thread = ctrl_tx.clone();
	let command_handle = thread::spawn(move || command_loop(command_stream, ctrl_tx_thread, cmd_stop, cmd_shutdown));

	drop(stream);

	let mut manual_mode = config.gain.is_some();
	let mut last_gain = config.gain.unwrap_or_else(|| gain_values.get(0).copied().unwrap_or(0));

	loop {
		if connection_stop.load(Ordering::Relaxed) || global_shutdown.load(Ordering::Relaxed) {
			break;
		}

		loop {
			match ctrl_rx.try_recv() {
				Ok(msg) => {
					let should_break = match handle_control_message(
						&mut sdr,
						msg,
						&mut manual_mode,
						&mut last_gain,
						&mut gain_values,
					) {
						Ok(flag) => flag,
						Err(err) => {
							outcome_error = Some(err);
							connection_stop.store(true, Ordering::SeqCst);
							true
						}
					};
					if should_break {
						connection_stop.store(true, Ordering::SeqCst);
						break;
					}
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => {
					connection_stop.store(true, Ordering::SeqCst);
					break;
				}
			}
		}

		if connection_stop.load(Ordering::Relaxed) || global_shutdown.load(Ordering::Relaxed) {
			break;
		}

		let mut buffer = vec![0u8; DEFAULT_BUF_LENGTH];
		match sdr.read_sync(&mut buffer[..]) {
			Ok(bytes) => {
				if bytes == 0 {
					outcome_error = Some("Device returned zero bytes".to_string());
					connection_stop.store(true, Ordering::SeqCst);
					break;
				}
				if bytes < buffer.len() {
					buffer.truncate(bytes);
				}
				if data_tx.send(buffer).is_err() {
					connection_stop.store(true, Ordering::SeqCst);
					break;
				}
			}
			Err(err) => {
				outcome_error = Some(format!("Read error: {}", err));
				connection_stop.store(true, Ordering::SeqCst);
				break;
			}
		}
	}

	drop(data_tx);
	let _ = ctrl_tx.send(ControlMessage::Shutdown);
	connection_stop.store(true, Ordering::SeqCst);

	let sender_result = sender_handle
		.join()
		.unwrap_or_else(|_| Err("sender thread panicked".to_string()));
	let command_result = command_handle
		.join()
		.unwrap_or_else(|_| Err("command thread panicked".to_string()));

	let mut errors: Vec<String> = Vec::new();
	if let Err(err) = sender_result {
		errors.push(err);
	}
	if let Err(err) = command_result {
		errors.push(err);
	}

	if let Some(err) = outcome_error.take() {
		errors.push(err);
	}

	let error = if errors.is_empty() {
		None
	} else {
		Some(errors.join(", "))
	};

	ServeOutcome { sdr, error }
}

fn handle_control_message(
	sdr: &mut RtlSdr,
	message: ControlMessage,
	manual_mode: &mut bool,
	last_gain: &mut i32,
	gain_values: &mut Vec<i32>,
) -> Result<bool, String> {
	match message {
		ControlMessage::SetFrequency(freq) => {
			sdr
				.set_center_freq(freq)
				.map_err(|e| format!("Failed to set frequency: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetSampleRate(rate) => {
			sdr
				.set_sample_rate(rate)
				.map_err(|e| format!("Failed to set sample rate: {}", e))?;
			sdr
				.reset_buffer()
				.map_err(|e| format!("Failed to reset buffer: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetGainMode(manual) => {
			*manual_mode = manual;
			if !manual {
				sdr
					.set_tuner_gain(TunerGain::Auto)
					.map_err(|e| format!("Failed to set auto gain: {}", e))?;
			} else {
                sdr
                    .set_tuner_gain(TunerGain::Manual(0))
                    .map_err(|e| format!("Failed to set manual gain: {}", e))?;
            }
			Ok(false)
		}
		ControlMessage::SetGain(gain) => {
			*manual_mode = true;
			*last_gain = gain;
			sdr
				.set_tuner_gain(TunerGain::Manual(gain))
				.map_err(|e| format!("Failed to set manual gain: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetGainByIndex(index) => {
			let gain = gain_values.get(index as usize).copied().or_else(|| {
				if let Ok(gains) = sdr.get_tuner_gains() {
					*gain_values = gains;
					gain_values.get(index as usize).copied()
				} else {
					None
				}
			});
			if let Some(gain) = gain {
				*manual_mode = true;
				*last_gain = gain;
				sdr
					.set_tuner_gain(TunerGain::Manual(gain))
					.map_err(|e| format!("Failed to set gain by index: {}", e))?;
			}
			Ok(false)
		}
		ControlMessage::SetFreqCorrection(ppm) => {
			sdr
				.set_freq_correction(ppm)
				.map_err(|e| format!("Failed to set PPM: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetIfGain { stage, gain } => {
			println!("set if gain not supported (stage={}, gain={})", stage, gain);
			Ok(false)
		}
		ControlMessage::SetTestMode(on) => {
			sdr
				.set_testmode(on)
				.map_err(|e| format!("Failed to set test mode: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetAgcMode(_on) => {
			println!("set agc mode not implemented");
			Ok(false)
		}
		ControlMessage::SetDirectSampling(mode) => {
			let ds_mode = match mode {
				0 => DirectSampleMode::Off,
				1 => DirectSampleMode::On,
				2 => DirectSampleMode::OnSwap,
				_ => DirectSampleMode::Off,
			};
			sdr
				.set_direct_sampling(ds_mode)
				.map_err(|e| format!("Failed to set direct sampling: {}", e))?;
			Ok(false)
		}
		ControlMessage::SetOffsetTuning(on) => {
			println!("offset tuning request ignored (not supported): {}", on);
			Ok(false)
		}
		ControlMessage::SetRtlXtal(freq) => {
			println!("set rtl xtal not supported: {}", freq);
			Ok(false)
		}
		ControlMessage::SetTunerXtal(freq) => {
			println!("set tuner xtal not supported: {}", freq);
			Ok(false)
		}
		ControlMessage::SetBiasTee(on) => {
			sdr
				.set_bias_tee(on)
				.map_err(|e| format!("Failed to set bias tee: {}", e))?;
			Ok(false)
		}
		ControlMessage::Shutdown => Ok(true),
	}
}

fn sender_loop(
	mut stream: TcpStream,
	data_rx: Receiver<Vec<u8>>,
	stop: Arc<AtomicBool>,
	global_shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
	loop {
		if stop.load(Ordering::Relaxed) || global_shutdown.load(Ordering::Relaxed) {
			break;
		}
		match data_rx.recv_timeout(Duration::from_millis(200)) {
			Ok(buf) => {
				if let Err(err) = stream.write_all(&buf) {
					stop.store(true, Ordering::SeqCst);
					return Err(format!("Failed to send data: {}", err));
				}
			}
			Err(RecvTimeoutError::Timeout) => continue,
			Err(RecvTimeoutError::Disconnected) => break,
		}
	}
	Ok(())
}

fn command_loop(
	mut stream: TcpStream,
	ctrl_tx: mpsc::Sender<ControlMessage>,
	stop: Arc<AtomicBool>,
	global_shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
	let mut buf = [0u8; 5];
	loop {
		if stop.load(Ordering::Relaxed) || global_shutdown.load(Ordering::Relaxed) {
			break;
		}

		if let Err(err) = stream.read_exact(&mut buf) {
			if err.kind() == io::ErrorKind::UnexpectedEof {
				break;
			} else {
				stop.store(true, Ordering::SeqCst);
				return Err(format!("Command read failed: {}", err));
			}
		}

		let cmd = buf[0];
		let param_bytes = [buf[1], buf[2], buf[3], buf[4]];
		let param_u32 = u32::from_be_bytes(param_bytes);
		let param_i32 = i32::from_be_bytes(param_bytes);

		let message = match cmd {
			0x01 => Some(ControlMessage::SetFrequency(param_u32)),
			0x02 => Some(ControlMessage::SetSampleRate(param_u32)),
			0x03 => Some(ControlMessage::SetGainMode(param_u32 != 0)),
			0x04 => Some(ControlMessage::SetGain(param_i32)),
			0x05 => Some(ControlMessage::SetFreqCorrection(param_i32)),
			0x06 => Some(ControlMessage::SetIfGain {
				stage: (param_u32 >> 16) as u16,
				gain: (param_u32 & 0xffff) as i16,
			}),
			0x07 => Some(ControlMessage::SetTestMode(param_u32 != 0)),
			0x08 => Some(ControlMessage::SetAgcMode(param_u32 != 0)),
			0x09 => Some(ControlMessage::SetDirectSampling(param_u32)),
			0x0a => Some(ControlMessage::SetOffsetTuning(param_u32 != 0)),
			0x0b => Some(ControlMessage::SetRtlXtal(param_u32)),
			0x0c => Some(ControlMessage::SetTunerXtal(param_u32)),
			0x0d => Some(ControlMessage::SetGainByIndex(param_u32)),
			0x0e => Some(ControlMessage::SetBiasTee(param_u32 != 0)),
			_ => None,
		};

		if let Some(msg) = message {
			if ctrl_tx.send(msg).is_err() {
				break;
			}
		}
	}

	let _ = ctrl_tx.send(ControlMessage::Shutdown);
	Ok(())
}

fn send_handshake(stream: &mut TcpStream, tuner_type: u32, gain_count: u32) -> io::Result<()> {
	let mut payload = [0u8; 12];
	payload[0..4].copy_from_slice(b"RTL0");
	payload[4..8].copy_from_slice(&tuner_type.to_be_bytes());
	payload[8..12].copy_from_slice(&gain_count.to_be_bytes());
	stream.write_all(&payload)
}

fn detect_tuner_type(_sdr: &RtlSdr) -> u32 {
	6 // R828D
}
