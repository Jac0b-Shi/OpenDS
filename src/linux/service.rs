//! Local desktop protocol. Stdin commands and stdout snapshots share the existing
//! bridge/monitor implementation; device callbacks never perform UI or file I/O.
use crate::{bridge::BridgeController, config::Config, report::ParsedState};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

fn emit(value: Value) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &value)?;
    out.write_all(b"\n")?;
    out.flush()
}
fn input(s: ParsedState) -> Value {
    json!({"sticks":[s.lx,s.ly,s.rx,s.ry],"triggers":[s.l2,s.r2],"buttons":[s.buttons0,s.buttons1,s.buttons2],
        "touch":[[s.touch1.active as u16,s.touch1.x,s.touch1.y],[s.touch2.active as u16,s.touch2.x,s.touch2.y]],
        "gyro":s.motion.gyro,"accel":s.motion.accel,"timestamp":s.motion.timestamp})
}
fn snapshot(controller: &BridgeController, cfg: &Config, graphics: bool) -> Value {
    let state = &controller.state;
    let m = &state.monitor;
    let now = m.display_time();
    let mut report = json!({"type":"state","now":now,"running":state.running.load(Acquire),"connected":state.connected.load(Acquire),
        "hidden":state.hidhide_healthy.load(Acquire),"capture":m.active.load(Acquire),"edge":m.edge.load(Relaxed),
        "counts":[state.frames_in.load(Relaxed),state.frames_out.load(Relaxed),state.audio_blocks.load(Relaxed)],
        "drops":m.dropped(),"decode_errors":m.decode_failures.load(Relaxed),"resets":m.resets.load(Relaxed),"cache":m.allocated_bytes(),
        "source":m.source.load(Relaxed),"forwarded":m.forwarded.load(Relaxed),"logs":state.logs(),
        "error":state.last_error.lock().ok().and_then(|e|e.clone()),"microphone":m.microphone.load(Relaxed),
        "mic_gain":m.microphone_gain_db.load(Relaxed),"mic_buffer":m.microphone_buffer_ms.load(Relaxed),
        "mic_underruns":m.mic_underruns.load(Relaxed),"mic_dropped":m.mic_dropped.load(Relaxed),"route":m.route.load(Relaxed),"volume":m.volume.load(Relaxed)});
    let battery = m.battery.snapshot().view(m.battery.now());
    report["battery"] =
        json!({"percent":battery.percent,"caption":battery.caption(),"tooltip":battery.tooltip()});
    let recording = m.recording.status();
    report["recording"] = json!({"active":recording.active,"starting":recording.starting,"finishing":recording.finishing,"rows":recording.rows,"dropped":recording.dropped,"path":recording.path,"error":recording.error.or_else(||m.commands.recording.error())});
    if graphics {
        if let Ok(mut h) = m.history.lock() {
            report["input"] = input(h.input);
            report["pressed"] = json!(h.pressed);
            h.pressed = [0; 3];
            report["input_age"] = json!(now.saturating_sub(h.input_time));
            report["waves"] = json!(h
                .plot(cfg.monitor_seconds, 240, now)
                .iter()
                .map(|b| json!([b.min, b.max, b.valid, b.gap]))
                .collect::<Vec<_>>());
            report["wave_states"] = json!((0..3)
                .map(|g| format!(
                    "{:?}",
                    crate::monitor::wave_state(
                        g,
                        [m.source.load(Relaxed), m.forwarded.load(Relaxed)],
                        &h.last,
                        &h.nonzero,
                        now
                    )
                ))
                .collect::<Vec<_>>());
        }
        if let Ok(h) = m.commands.history.lock() {
            let mut events: Vec<_> = h.events.iter().rev().cloned().collect();
            if let Some(latest) = h.visible_latest() {
                if events
                    .first()
                    .is_none_or(|e| e.session != latest.session || e.sequence != latest.sequence)
                {
                    events.insert(0, latest.clone());
                }
            }
            events.truncate(64);
            report["events"]=json!(events.iter().enumerate().map(|(i,e)|json!({"id":format!("{}-{}",e.session,e.sequence),"kind":e.kind.name(),"combination":crate::commands::combination(e.state,h.settings.right),"duration":crate::commands::duration(crate::commands::state_duration(&events,i,now)),"raw":e.raw,"strength":[e.state.sticks[0].strength,e.state.sticks[1].strength,e.state.triggers[0],e.state.triggers[1]]})).collect::<Vec<_>>());
        }
    }
    report
}

pub fn run() -> Result<(), String> {
    let _instance = crate::persistence::InstanceGuard::acquire().map_err(|e| e.to_string())?;
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = super::stop as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
    }
    let path = Config::path();
    let mut cfg = Config::load(&path)?;
    if !path.exists() {
        cfg.save(&path)?;
    }
    let mut controller = BridgeController::default();
    controller
        .state
        .monitor
        .commands
        .configure(cfg.commands.clone(), 0);
    controller
        .state
        .monitor
        .commands
        .visible_capacity(false, 64);
    controller.state.monitor.commands.visible_capacity(true, 64);
    let strings: serde_json::Map<String, Value> = crate::i18n::english_catalog()
        .variables
        .into_iter()
        .filter(|(_, vars)| vars.is_empty())
        .map(|(key, _)| {
            let value = crate::i18n::current().format_key(&key, None);
            (key, Value::String(value))
        })
        .collect();
    let mut english_bundle = fluent_bundle::FluentBundle::new(vec!["en-US"
        .parse::<unic_langid::LanguageIdentifier>()
        .unwrap()]);
    english_bundle
        .add_resource(
            fluent_bundle::FluentResource::try_new(include_str!("../../locales/en-US.ftl").into())
                .unwrap(),
        )
        .unwrap();
    let english_strings: serde_json::Map<String, Value> = strings
        .keys()
        .map(|key| {
            let value = english_bundle
                .get_message(key)
                .and_then(|message| message.value())
                .map(|pattern| {
                    english_bundle
                        .format_pattern(pattern, None, &mut vec![])
                        .into_owned()
                })
                .unwrap_or_else(|| key.clone());
            (key.clone(), Value::String(value))
        })
        .collect();
    let languages:Vec<_>=crate::i18n::languages_in(&crate::i18n::directory()).iter().map(|l|json!({"code":l.code,"name":l.name,"local":l.local,"translated":l.translated,"total":l.total})).collect();
    emit(json!({"type":"ready","version":env!("CARGO_PKG_VERSION"),"strings":strings,"english_strings":english_strings,"languages":languages,"config":path,"locale":crate::i18n::current().code})).map_err(|e|e.to_string())?;
    let (tx, rx) = mpsc::sync_channel::<Value>(64);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(line) if line.len() <= 1024 * 1024 => {
                    let request = serde_json::from_str(&line)
                        .unwrap_or_else(|e| json!({"op":"invalid","error":e.to_string()}));
                    if tx.send(request).is_err() {
                        return;
                    }
                }
                _ => break,
            }
        }
        let _ = tx.send(json!({"op":"quit"}));
    });
    let (mic_tx, mic_rx) = mpsc::channel();
    let mut mic_worker = None;
    let mic_cancel = Arc::new(AtomicBool::new(false));
    let mut quit = false;
    let mut graphics = true;
    let mut sent = Instant::now() - Duration::from_secs(1);
    while !quit && !super::STOP.load(Acquire) {
        if let Ok(request) = rx.recv_timeout(Duration::from_millis(5)) {
            let id = request["id"].clone();
            let op = request["op"].as_str().unwrap_or("");
            let result = (|| -> Result<Value, String> {
                let m = &controller.state.monitor;
                match op {
                    "bridge_start"=>{ cfg=Config::load(&path)?; controller.start(cfg.clone(),None)?; Ok(json!(true)) },
                    "bridge_stop"=>{controller.stop();Ok(json!(true))},
                    "capture_start"=>{m.start(cfg.monitor_sample_hz)?;Ok(json!(true))},
                    "capture_stop"=>{m.stop();Ok(json!(true))},
                    "clear"=>{m.clear();Ok(json!(true))},
                    "history_clear"=>{m.commands.clear(m.now());Ok(json!(true))},
                    "record_start"=>{if !m.active.load(Acquire){return Err("Start capture before recording".into());}m.start_recording(PathBuf::from(request["path"].as_str().ok_or("Missing CSV path")?))?;Ok(json!(true))},
                    "record_stop"=>{m.stop_recording();Ok(json!(true))},
                    "signal"=>{m.signal(request["forwarded"].as_bool().unwrap_or(false),request["enabled"].as_bool().unwrap_or(false));Ok(json!(true))},
                    "visible"=>{graphics=request["value"].as_bool().unwrap_or(true);Ok(json!(true))},
                    "configure"=>{
                        let candidate=Config::parse(request["text"].as_str().ok_or("Missing settings")?)?;candidate.validate()?;candidate.save(&path)?;
                        m.commands.configure(candidate.commands.clone(),m.now());
                        m.microphone_gain_db.store(candidate.microphone_gain_db.into(),Release);
                        m.microphone_buffer_ms.store(candidate.microphone_buffer_ms.into(),Release);
                        if m.active.load(Acquire) && candidate.monitor_sample_hz!=cfg.monitor_sample_hz {m.start(candidate.monitor_sample_hz)?;}
                        cfg=candidate;Ok(json!(true))
                    },
                    "doctor"=>Ok(json!(crate::dependencies::summary(&cfg))),
                    "devices"=>Ok(json!(crate::hid::HidDevice::enum_paths().iter().map(|p|json!({"path":p,"accessible":crate::hid::HidDevice::open(p).is_some()})).collect::<Vec<_>>())),
                    "language_import"=>{let lang=crate::i18n::import(std::path::Path::new(request["path"].as_str().ok_or("Missing language file")?),&crate::i18n::directory()).map_err(|e|e.localized())?;Ok(json!({"code":lang.code,"name":lang.name,"translated":lang.translated,"total":lang.total}))},
                    "mic_capture"=>{
                        if !controller.state.connected.load(Acquire)||!cfg.microphone_enabled{return Err("Start the bridge with microphone forwarding enabled first".into());}
                        if mic_worker.as_ref().is_some_and(|w:&std::thread::JoinHandle<()>|!w.is_finished()){return Err("Microphone test is already recording".into());}
                        if let Some(w)=mic_worker.take(){let _=w.join();}
                        mic_cancel.store(false,Release);let cancel=mic_cancel.clone();let source=m.mic_test_capture.clone();let tx=mic_tx.clone();let generation=request["generation"].as_u64().unwrap_or(0);
                        mic_worker=Some(std::thread::spawn(move||{
                            let result=(||->Result<Value,String>{
                                for _ in 0..150 {if cancel.load(Acquire){return Err("Microphone test cancelled".into());}std::thread::sleep(Duration::from_millis(20));}
                                let recording=source.begin();
                                for _ in 0..250{if cancel.load(Acquire){break;}std::thread::sleep(Duration::from_millis(20));}
                                let captured=recording.finish();if cancel.load(Acquire){return Err("Microphone test cancelled".into());}
                                if captured.pcm.is_empty(){return Err("No Bluetooth microphone audio received".into());}
                                let output=super::state_dir().join("microphone-test-bluetooth.wav");write_wav(&output,&captured.pcm)?;
                                Ok(json!({"path":output,"frames":captured.pcm.len()/2,"dropped":captured.dropped,"gap_ms":captured.longest_gap_ms}))
                            })();let _=tx.send((generation,result));
                        }));Ok(json!(true))
                    },
                    "mic_cancel"=>{mic_cancel.store(true,Release);if let Some(worker)=mic_worker.take(){let _=worker.join();}let _=std::fs::remove_file(super::state_dir().join("microphone-test-bluetooth.wav"));Ok(json!(true))},
                    "quit"=>{quit=true;Ok(json!(true))},
                    _=>Err(format!("Unknown desktop operation: {op}")),
                }
            })();
            let response = match result {
                Ok(value) => json!({"type":"reply","id":id,"ok":true,"value":value}),
                Err(error) => json!({"type":"reply","id":id,"ok":false,"error":error}),
            };
            if emit(response).is_err() {
                quit = true;
            }
        }
        controller.poll();
        if let Ok((generation, result)) = mic_rx.try_recv() {
            let event = match result {
                Ok(value) => {
                    json!({"type":"microphone","generation":generation,"ok":true,"value":value})
                }
                Err(error) => {
                    json!({"type":"microphone","generation":generation,"ok":false,"error":error})
                }
            };
            if emit(event).is_err() {
                quit = true;
            }
        }
        let interval = if graphics {
            Duration::from_millis(1000 / cfg.monitor_fps as u64)
        } else {
            Duration::from_secs(1)
        };
        if sent.elapsed() >= interval {
            if emit(snapshot(&controller, &cfg, graphics)).is_err() {
                quit = true;
            }
            sent = Instant::now();
        }
    }
    mic_cancel.store(true, Release);
    controller.stop();
    controller.wait();
    controller.state.monitor.stop();
    controller.state.monitor.recording.finish();
    controller.state.monitor.commands.recording.finish();
    if let Some(worker) = mic_worker {
        let _ = worker.join();
    }
    let _ = std::fs::remove_file(super::state_dir().join("microphone-test-bluetooth.wav"));
    Ok(())
}

fn write_wav(path: &std::path::Path, pcm: &[i16]) -> Result<(), String> {
    let bytes = (pcm.len() * 2) as u32;
    let mut data = Vec::with_capacity(bytes as usize + 44);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + bytes).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&48000u32.to_le_bytes());
    data.extend_from_slice(&192000u32.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&bytes.to_le_bytes());
    for sample in pcm {
        data.extend_from_slice(&sample.to_le_bytes());
    }
    crate::persistence::atomic_write(path, &data).map_err(|e| e.to_string())
}
