mod bigmap;
mod console;
mod effects;
mod hud;
mod menu;
mod play;
mod snowtracks;
mod sound;
mod weatherfx;
mod wheel;
use anyhow::{Context, Result, ensure};
use berlin_engine::Options;
use berlin_map_loader::{
    format::{Index, Tile, TileKey},
    projection::geo_to_px,
};
fn main() -> Result<()> {
    let mut options = Options {
        fps: 60,
        smoke_frames: None,
        data_root: berlin_map_loader::default_data_root(),
        position: None,
        lighting: Default::default(),
        capture: None,
        zoom: 2.,
    };
    let mut args = std::env::args().skip(1);
    let mut geo = None;
    let mut check_map = false;
    let mut free = false;
    let mut new_game = false;
    let mut resume = false;
    let mut screen: Option<String> = None;
    let mut demo_combat = false;
    let mut vehicle_show = false;
    let mut seed = 1989u32;
    let mut check_sim: Option<f64> = None;
    let mut save_path = None;
    let mut clock: Option<f64> = None;
    let mut sound = true;
    let mut in_car = false;
    let mut force_weather: Option<&'static str> = None;
    let mut stadtplan: Option<f32> = None;
    let mut bars: Option<String> = None;
    let mut commands: Vec<String> = Vec::new();
    let mut audio_wav: Option<std::path::PathBuf> = None;
    let mut audio_secs = 20.;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fps" => options.fps = args.next().context("Wert für --fps fehlt")?.parse()?,
            "--smoke-frames" => {
                let frames = args
                    .next()
                    .context("Wert für --smoke-frames fehlt")?
                    .parse()?;
                ensure!(frames > 0, "Framezahl muss positiv sein");
                options.smoke_frames = Some(frames);
            }
            "--data" => options.data_root = args.next().context("Pfad für --data fehlt")?.into(),
            "--position" => {
                let x = args.next().context("X fehlt")?.parse()?;
                let y = args.next().context("Y fehlt")?.parse()?;
                options.position = Some([x, y].into());
            }
            "--geo" => {
                let lat: f64 = args.next().context("Breite fehlt")?.parse()?;
                let lon: f64 = args.next().context("Länge fehlt")?.parse()?;
                ensure!(
                    lat.is_finite()
                        && lon.is_finite()
                        && (-90.0..90.0).contains(&lat)
                        && (-180.0..=180.0).contains(&lon),
                    "Ungültige Geokoordinaten"
                );
                geo = Some((lat, lon));
            }
            "--sun-hour" => {
                let h: f64 = args.next().context("Stunde fehlt")?.parse()?;
                ensure!(
                    (0.0..24.0).contains(&h),
                    "Stunde muss zwischen 0 und 24 liegen"
                );
                clock = Some(h * 60.);
            }
            "--uhr" | "--clock" => {
                let text = args.next().context("Uhrzeit HH:MM fehlt")?;
                clock = Some(
                    berlin_sim::daylight::parse_clock(&text)
                        .with_context(|| format!("Ungültige Uhrzeit: {text}"))?,
                );
            }
            "--zoom" => {
                options.zoom = args.next().context("Zoom fehlt")?.parse()?;
                ensure!(
                    (0.72..=2.6).contains(&options.zoom),
                    "Zoom muss zwischen 0.72 und 2.6 liegen"
                );
            }
            "--capture" => {
                options.capture = Some(args.next().context("PNG-Pfad fehlt")?.into());
            }
            "--check-map" => check_map = true,
            "--stumm" | "--mute" => sound = false,
            "--im-auto" => in_car = true,
            "--befehl" => commands.push(args.next().context("Befehl für --befehl fehlt")?),
            "--bars" => bars = Some(args.next().context("Datei für --bars fehlt (oder aus)")?),
            "--stadtplan" => {
                let z: f32 = args
                    .next()
                    .context("Zoom für --stadtplan fehlt (1 = ganz Berlin)")?
                    .parse()?;
                ensure!(
                    (1.0..=bigmap::ZOOM_MAX).contains(&z),
                    "--stadtplan erwartet 1..64"
                );
                stadtplan = Some(z);
            }
            "--wetter" | "--weather" => {
                let k = args.next().context("Wetterart fehlt")?;
                force_weather = Some(
                    *berlin_sim::weather::KINDS
                        .iter()
                        .find(|x| **x == k)
                        .with_context(|| {
                            format!(
                                "Unbekanntes Wetter {k} – erlaubt: {}",
                                berlin_sim::weather::KINDS.join(", ")
                            )
                        })?,
                );
            }
            "--audio-wav" => {
                audio_wav = Some(args.next().context("Pfad für --audio-wav fehlt")?.into())
            }
            "--audio-seconds" => {
                audio_secs = args.next().context("Sekunden fehlen")?.parse()?;
                ensure!(
                    audio_secs > 0. && audio_secs <= 600.,
                    "--audio-seconds: 0 < s ≤ 600"
                );
            }
            "--free" => free = true,
            "--new" => new_game = true,
            "--fortsetzen" => resume = true,
            "--kampf-demo" => demo_combat = true,
            "--fahrzeugschau" => vehicle_show = true,
            "--bildschirm" => {
                let v = args.next().context(
                    "--bildschirm erwartet pause, steuerung, statistik, waffenrad, teleport, konsole oder zugfahrt",
                )?;
                ensure!(
                    [
                        "pause",
                        "steuerung",
                        "statistik",
                        "waffenrad",
                        "teleport",
                        "konsole",
                        "zugfahrt"
                    ]
                    .contains(&v.as_str()),
                    "--bildschirm erwartet pause, steuerung, statistik, waffenrad, teleport, konsole oder zugfahrt"
                );
                screen = Some(v);
            }
            "--seed" => seed = args.next().context("Wert für --seed fehlt")?.parse()?,
            "--save" => {
                save_path = Some(std::path::PathBuf::from(
                    args.next().context("Pfad für --save fehlt")?,
                ))
            }
            "--check-sim" => {
                let secs: f64 = args
                    .next()
                    .context("Sekunden für --check-sim fehlen")?
                    .parse()?;
                ensure!(
                    secs > 0. && secs <= 3600.,
                    "--check-sim braucht 0 < Sekunden ≤ 3600"
                );
                check_sim = Some(secs);
            }
            "--help" | "-h" => {
                println!(
                    "cargo run -- [--fps 60|120] [--data PFAD] [--seed N] [--new | --fortsetzen] [--save DATEI] [--free [--position X Y | --geo LAT LON] [--zoom 0.72..2.6]] [--uhr HH:MM | --sun-hour 0..24] [--smoke-frames N] [--capture PNG] [--check-map] [--check-sim SEKUNDEN] [--stumm] [--im-auto] [--wetter ART] [--stadtplan ZOOM] [--bildschirm pause|steuerung|statistik|waffenrad|teleport|konsole|zugfahrt] [--bars DATEI|aus] [--befehl BEFEHL] [--kampf-demo] [--fahrzeugschau] [--audio-wav DATEI [--audio-seconds N]]\n\
Spiel: WASD/Pfeile gehen bzw. Gas/Bremse/Lenken · Shift: sprinten · Alt: langsam · F: ein-/aussteigen · E: Aktion (halten: einladen) · Leertaste: Handbremse · H: Hupe · X: ESP · Y/Z: ABS · T: +1 Stunde · N: Wetter durchschalten · M: Ton an/aus · Tab: Stadtplan · Strg: angreifen · V: treten · Q/1–6: Waffe · R: nachladen · F5: speichern · Mausrad: Zoom · Esc/P: Pause (Menü: Beenden)\n\
--free: freie Kartenansicht wie in Phase 2 (WASD/Shift/Mausrad, 1/2/3 Zoomstufen)"
                );
                return Ok(());
            }
            _ => anyhow::bail!("Unbekanntes Argument: {arg}"),
        }
    }
    if options.capture.is_some() && options.smoke_frames.is_none() {
        options.smoke_frames = Some(10);
    }
    let index = Index::read(&options.data_root)?;
    if check_map {
        let mut objects = 0;
        let mut polygons = 0;
        for key in &index.tiles {
            let tile = Tile::read(&options.data_root, TileKey::parse(key)?, &index.meta)?;
            for item in &tile.items {
                let polygon = match &item.feature {
                    berlin_map_loader::format::Feature::Building(b) => Some(&b.polygon),
                    berlin_map_loader::format::Feature::Area { polygon, .. }
                    | berlin_map_loader::format::Feature::Water { polygon, .. } => Some(polygon),
                    _ => None,
                };
                if let Some(polygon) = polygon {
                    polygons += 1;
                    berlin_map_loader::mesh::triangulate(polygon)
                        .with_context(|| format!("Kachel {key}, Objekt {:?}", item.id))?;
                }
            }
            objects += tile.items.len();
        }
        println!(
            "Kartenformat v3: {} Kacheln dekodiert, {polygons} Polygone trianguliert, {objects} Einträge (einschließlich geteilter Objekte)",
            index.tiles.len()
        );
        return Ok(());
    }
    if let Some(path) = audio_wav {
        return render_audio(&options.data_root, seed, audio_secs, &path);
    }
    if let Some(secs) = check_sim {
        return check_simulation(&options.data_root, seed, secs);
    }
    if let Some((lat, lon)) = geo {
        ensure!(
            options.position.is_none(),
            "--geo und --position schließen sich aus"
        );
        options.position = Some(geo_to_px(&index.meta, lat, lon).as_vec2());
    }
    if let Some(position) = options.position {
        ensure!(
            position.is_finite()
                && position.x >= 0.
                && position.y >= 0.
                && position.x <= index.meta.width
                && position.y <= index.meta.height,
            "Startposition außerhalb der Karte"
        );
    }
    options.lighting = play::lighting_at(clock.unwrap_or(13. * 60.));
    if free {
        return berlin_engine::run(options);
    }
    ensure!(
        options.position.is_none(),
        "--position/--geo gelten nur mit --free"
    );
    let storage = save_path
        .map(berlin_sim::save::FileStorage::new)
        .unwrap_or_else(|| {
            berlin_sim::save::FileStorage::new(berlin_sim::save::FileStorage::default_path())
        });
    // Start: Titelbildschirm; --new beginnt sofort neu, --fortsetzen lädt den Stand (Aufnahme-/Testoptionen
    // wie --im-auto und --stadtplan springen ebenfalls direkt ins Spiel)
    let start = if new_game {
        play::Start::New
    } else if resume
        || in_car
        || stadtplan.is_some()
        || screen.is_some()
        || demo_combat
        || vehicle_show
    {
        play::Start::Continue
    } else {
        play::Start::Title
    };
    let play = play::Play::new(&options.data_root, seed, Some(storage), sound, start)?;
    let mut play = play;
    play.demo_combat = demo_combat;
    play.vehicle_show = vehicle_show;
    match screen.as_deref() {
        Some("pause") => play.pause(),
        Some("steuerung") => play.screen = play::Screen::Controls(false),
        Some("statistik") => play.screen = play::Screen::Stats(false),
        Some("waffenrad") => play.demo_wheel(),
        Some("teleport") => play.demo_teleport(),
        Some("zugfahrt") => play.demo_drive = true,
        Some("konsole") => {
            play.console.open(&play.places);
            play.console.set_text("tp kott", &play.places);
        }
        _ => {}
    }
    if let Some(b) = bars {
        play.bars_file = (b != "aus").then(|| b.into());
        match play.reload_bars() {
            Ok(m) => eprintln!("Nachtleben: {m}"),
            Err(e) => anyhow::bail!("Bar-Feed nicht ladbar: {e}"),
        }
    }
    for c in &commands {
        let r = play.run_command(c);
        eprintln!("Befehl „{c}“: {}", r.msg);
        anyhow::ensure!(r.ok, "Befehl „{c}“ gescheitert");
    }
    play.auto_enter = in_car;
    play.world.force_weather = force_weather;
    // Aufnahmen: der Boden ist schon so nass bzw. verschneit, wie das erzwungene Wetter es nach einer Weile wäre
    if let Some(k) = force_weather {
        let g = &mut play.world.weather;
        match k {
            "rain" | "heavyrain" | "storm" | "thunder" => g.wet = 1.,
            "snow" | "heavysnow" => g.snow = 0.8,
            _ => {}
        }
    }
    if let Some(z) = stadtplan {
        // Karte offen, bei Zoom > 1 um den Spieler
        play.bigmap.open = true;
        play.bigmap.z = z;
        if z > 1. {
            play.bigmap.center =
                glam::Vec2::new(play.world.player.x as f32, play.world.player.y as f32);
        }
    }
    if let Some(c) = clock {
        play.world.clock = c;
    }
    berlin_engine::run_with(options, Some(Box::new(play)))
}

/// Simulation ohne Fenster laufen lassen und Kennzahlen ausgeben (Verkehr, Passanten, Mission, Tempo).
fn check_simulation(root: &std::path::Path, seed: u32, secs: f64) -> Result<()> {
    use berlin_sim::car::Driver;
    use berlin_sim::city::{City, DiskSource};
    use berlin_sim::world::{DT, Input, World, describe};
    let t0 = std::time::Instant::now();
    let city = City::open(root, Box::new(DiskSource::new(root)))?;
    let mut w = World::new(
        city,
        seed,
        berlin_sim::world::TRAFFIC_CARS,
        berlin_sim::world::TRAFFIC_PEDS,
    );
    let load = t0.elapsed();
    let steps = (secs / DT).round() as usize;
    let t1 = std::time::Instant::now();
    let mut crashes = 0;
    for _ in 0..steps {
        w.update(&Input::default(), DT);
        crashes += w
            .events
            .iter()
            .filter(|e| matches!(e, berlin_sim::events::Event::Crash { .. }))
            .count();
        for c in &w.cars {
            ensure!(
                c.x.is_finite() && c.y.is_finite(),
                "Auto {} hat ungültige Lage",
                c.id
            );
        }
    }
    let run = t1.elapsed();
    let npc = w.cars.iter().filter(|c| c.driver == Some(Driver::Npc));
    let moving = npc.clone().filter(|c| c.speed() > 20.).count();
    println!(
        "Simulation: {:.0} s Spielzeit in {:.2} s ({:.3} ms je Schritt), Laden {:.2} s · {} · {moving}/{} KI-Autos fahren · {crashes} Unfälle",
        secs,
        run.as_secs_f64(),
        run.as_secs_f64() * 1000. / steps as f64,
        load.as_secs_f64(),
        describe(&w),
        npc.count()
    );
    Ok(())
}

/// Eine Fahrt ohne Fenster simulieren und den Klang als WAV schreiben (zum Prüfen ohne Lautsprecher):
/// einsteigen, beschleunigen, bremsen, mit Einschlag weiterfahren.
fn render_audio(
    root: &std::path::Path,
    seed: u32,
    secs: f64,
    path: &std::path::Path,
) -> Result<()> {
    use berlin_audio::synth::Synth;
    use berlin_sim::city::{City, DiskSource};
    use berlin_sim::world::{DT, Input, World};
    const SR: u32 = 48000;
    let city = City::open(root, Box::new(DiskSource::new(root)))?;
    let mut w = World::new(
        city,
        seed,
        berlin_sim::world::TRAFFIC_CARS,
        berlin_sim::world::TRAFFIC_PEDS,
    );
    let pc = w.player_car_id.context("Kein Spielerauto")?;
    // Messfahrt auf der ersten langen geraden Spur nahe dem Späti (der Parkplatz ist zum Beschleunigen zu eng)
    let (px, py) = (w.player.x, w.player.y);
    let lane = w
        .lanes
        .lanes
        .values()
        .filter(|l| {
            (l.pts[1].0 - l.pts[0].0).hypot(l.pts[1].1 - l.pts[0].1) > 500.
                && (l.pts[0].0 - px).hypot(l.pts[0].1 - py) < 3000.
        })
        .min_by_key(|l| l.id)
        .context("Keine gerade Spur")?
        .clone();
    let (a, b) = (lane.pts[0], lane.pts[1]);
    if let Some(c) = w.cars.iter_mut().find(|c| c.id == pc) {
        (c.x, c.y, c.angle) = (a.0, a.1, (b.1 - a.1).atan2(b.0 - a.0));
    }
    w.cars
        .retain(|c| c.id == pc || (c.x - a.0).hypot(c.y - a.1) > 300.);
    (w.player.x, w.player.y) = (a.0, a.1);
    let mut synth = Synth::new(SR as f32);
    let mut listener = sound::Listener::default();
    let mut out = Vec::with_capacity((secs * SR as f64) as usize * 2);
    let steps = (secs / DT) as usize;
    let mut chunk = vec![0f32; 0];
    let mut acc = 0f64;
    for k in 0..steps {
        let t = k as f64 * DT;
        let input = match t {
            t if t < 1. => Input {
                enter_exit: k == 30,
                ..Default::default()
            },
            t if t < 7. => Input {
                throttle: 1.,
                ..Default::default()
            },
            t if t < 9. => Input {
                brake: 1.,
                ..Default::default()
            },
            _ => Input {
                throttle: 0.5,
                ..Default::default()
            },
        };
        if k % 60 == 0 {
            let e = listener.engine();
            eprintln!(
                "{t:4.1} s · {:3.0} km/h · {:4.0} U/min · Gang {} · Zündton {:3.0} Hz",
                w.car(pc).map(|c| c.speed() * 0.36).unwrap_or(0.),
                e.rpm,
                e.gear,
                e.fire
            );
        }
        w.update(&input, DT);
        let frame = listener.frame(&mut w, DT);
        synth.apply(&frame);
        acc += DT * SR as f64;
        let n = acc.floor() as usize;
        acc -= n as f64;
        chunk.resize(n * 2, 0.);
        synth.render(&mut chunk);
        out.extend_from_slice(&chunk);
    }
    berlin_audio::output::write_wav(path, &out, SR)?;
    let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len().max(1) as f32).sqrt();
    let peak = out.iter().fold(0f32, |m, v| m.max(v.abs()));
    let kmh = w.car(pc).map(|c| c.speed() * 0.36).unwrap_or(0.);
    println!(
        "Klang: {} ({:.1} s, {} Hz) · RMS {:.3} · Spitze {:.2} · Ende {:.0} km/h",
        path.display(),
        out.len() as f64 / 2. / SR as f64,
        SR,
        rms,
        peak,
        kmh
    );
    Ok(())
}
