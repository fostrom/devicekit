use sysinfo::Components;

pub fn fetch_temperature(components: &Components) -> Option<f32> {
    let candidates = components
        .iter()
        .map(|component| {
            let id = component.id().unwrap_or_default();
            (component.label(), id, component.temperature())
        })
        .collect();

    select_temperature_from_candidates(candidates)
}

fn select_temperature_from_candidates(candidates: Vec<(&str, &str, Option<f32>)>) -> Option<f32> {
    let candidates = candidates
        .into_iter()
        .filter_map(|(label, id, temperature)| {
            let temperature = if temperature?.is_finite() {
                temperature.unwrap()
            } else {
                0.0
            };

            let score = temperature_score(label, id);
            Some((score, temperature))
        })
        .collect::<Vec<_>>();

    let best_score = candidates
        .iter()
        .map(|(score, _)| *score)
        .max()
        .unwrap_or(0);

    let temperatures = if best_score > 0 {
        candidates
            .iter()
            .filter(|(score, _)| *score == best_score)
            .map(|(_, temperature)| *temperature)
            .collect::<Vec<_>>()
    } else {
        candidates
            .iter()
            .map(|(_, temperature)| *temperature)
            .collect::<Vec<_>>()
    };

    median_temperature(temperatures)
}

fn temperature_score(label: &str, id: &str) -> u8 {
    let text = format!("{label} {id}").to_ascii_lowercase();

    if text.contains("package") {
        100
    } else if text.contains("tdie") || text.contains("tctl") {
        95
    } else if text.contains("cpu") {
        90
    } else if text.contains("soc") {
        80
    } else if text.contains("die") {
        75
    } else if text.contains("coretemp") {
        70
    } else {
        0
    }
}

fn median_temperature(mut temperatures: Vec<f32>) -> Option<f32> {
    if temperatures.is_empty() {
        return None;
    }

    temperatures.sort_by(f32::total_cmp);
    let midpoint = temperatures.len() / 2;

    if temperatures.len().is_multiple_of(2) {
        Some((temperatures[midpoint - 1] + temperatures[midpoint]) / 2.0)
    } else {
        Some(temperatures[midpoint])
    }
}
