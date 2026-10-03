mod play;
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
        sun_hour: 13.,
        capture: None,
        zoom: 2.,
    };
    let mut args = std::env::args().skip(1);
    let mut geo = None;
    let mut check_map = false;
    let mut free = false;
    let mut new_game = false;
    let mut seed = 1989u32;
    let mut check_sim: Option<f64> = None;
    let mut save_path = None;
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
                options.sun_hour = args.next().context("Stunde fehlt")?.parse()?;
                ensure!(
                    (5.5..=20.5).contains(&options.sun_hour),
                    "Sonnenstunde muss zwischen 5.5 und 20.5 liegen"
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
            "--free" => free = true,
            "--new" => new_game = true,
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
                    "cargo run -- [--fps 60|120] [--data PFAD] [--seed N] [--new] [--save DATEI] [--free [--position X Y | --geo LAT LON] [--zoom 0.72..2.6]] [--sun-hour 5.5..20.5] [--smoke-frames N] [--capture PNG] [--check-map] [--check-sim SEKUNDEN]\n\
Spiel: WASD/Pfeile gehen bzw. Gas/Bremse/Lenken · Shift: sprinten · Alt: langsam · F: ein-/aussteigen · E: Aktion (halten: einladen) · Leertaste: Handbremse · H: Hupe · X: ESP · Y/Z: ABS · F5: speichern · Mausrad: Zoom · Esc: Ende\n\
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
    let play = if new_game {
        // neues Spiel: vorhandenen Stand nicht laden, aber beim Speichern überschreiben
        let mut p = play::Play::new(&options.data_root, seed, None)?;
        p.set_storage(storage);
        p
    } else {
        play::Play::new(&options.data_root, seed, Some(storage))?
    };
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
