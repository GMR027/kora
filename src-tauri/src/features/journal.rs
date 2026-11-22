use chrono::NaiveDate;
use serde::Serialize;

use crate::storage::DiaryEntry;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryDay {
    pub date: String,
    pub minutes: i64,
    pub pages: i64,
    pub extracts: i64,
    pub notes: i64,
    pub entries: usize,
    /// Intensidad del heatmap: 0 (nada), 1..4.
    pub level: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryTotals {
    pub minutes: i64,
    pub pages: i64,
    pub extracts: i64,
    pub notes: i64,
    pub entries: usize,
    pub active_days: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryMonth {
    pub year: i32,
    pub month: u32,
    pub days: Vec<DiaryDay>,
    pub entries: Vec<DiaryEntry>,
    pub totals: DiaryTotals,
}

/// Agrupa las entradas por día y rellena todos los días del mes para que el
/// frontend pueda pintar el heatmap sin lógica de calendario.
pub fn aggregate_month(year: i32, month: u32, entries: Vec<DiaryEntry>) -> DiaryMonth {
    let first = NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 1, 1).expect("fecha mínima válida"));
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap_or(first);
    let last = next.pred_opt().unwrap_or(first);

    let mut days = Vec::new();
    let mut cursor = first;
    while cursor <= last {
        days.push(DiaryDay {
            date: cursor.format("%Y-%m-%d").to_string(),
            minutes: 0,
            pages: 0,
            extracts: 0,
            notes: 0,
            entries: 0,
            level: 0,
        });
        let Some(next_day) = cursor.succ_opt() else {
            break;
        };
        cursor = next_day;
    }

    let mut totals = DiaryTotals {
        minutes: 0,
        pages: 0,
        extracts: 0,
        notes: 0,
        entries: entries.len(),
        active_days: 0,
    };

    for entry in &entries {
        totals.minutes += entry.duration_minutes.max(0);
        totals.pages += entry.pages_read.max(0);
        totals.extracts += entry.extracts_created.max(0);
        totals.notes += entry.notes_created.max(0);

        if let Some(day) = days.iter_mut().find(|day| day.date == entry.entry_date) {
            day.minutes += entry.duration_minutes.max(0);
            day.pages += entry.pages_read.max(0);
            day.extracts += entry.extracts_created.max(0);
            day.notes += entry.notes_created.max(0);
            day.entries += 1;
        }
    }

    for day in &mut days {
        day.level = heat_level(day.minutes);
        if day.entries > 0 {
            totals.active_days += 1;
        }
    }

    DiaryMonth {
        year,
        month,
        days,
        entries,
        totals,
    }
}

pub fn heat_level(minutes: i64) -> u8 {
    match minutes {
        ..=0 => 0,
        1..=29 => 1,
        30..=59 => 2,
        60..=119 => 3,
        _ => 4,
    }
}

/// Primer y último día del mes en formato `YYYY-MM-DD`.
pub fn month_bounds(year: i32, month: u32) -> (String, String) {
    let first = NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 1, 1).expect("fecha mínima válida"));
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap_or(first);
    let last = next.pred_opt().unwrap_or(first);
    (
        first.format("%Y-%m-%d").to_string(),
        last.format("%Y-%m-%d").to_string(),
    )
}

/// Día actual en hora local, que es la que usa el diario.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn year_bounds(year: i32) -> (String, String) {
    (format!("{year}-01-01"), format!("{year}-12-31"))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::storage::DiaryKind;

    fn entry(date: &str, minutes: i64, pages: i64) -> DiaryEntry {
        DiaryEntry {
            id: date.to_string(),
            book_id: None,
            entry_date: date.to_string(),
            body: String::new(),
            mood: None,
            kind: DiaryKind::Reading,
            session_start: None,
            session_end: None,
            duration_minutes: minutes,
            pages_read: pages,
            extracts_created: 0,
            notes_created: 0,
            pomodoro_round: None,
            created_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            owner_account_id: "owner".to_string(),
            origin_share_id: None,
        }
    }

    #[test]
    fn aggregate_fills_every_day_and_sums_entries() {
        let month = aggregate_month(
            2026,
            3,
            vec![
                entry("2026-03-03", 10, 4),
                entry("2026-03-03", 25, 6),
                entry("2026-03-31", 120, 20),
            ],
        );

        assert_eq!(month.days.len(), 31);
        assert_eq!(month.days[2].minutes, 35);
        assert_eq!(month.days[2].level, 2);
        assert_eq!(month.days[30].level, 4);
        assert_eq!(month.totals.minutes, 155);
        assert_eq!(month.totals.pages, 30);
        assert_eq!(month.totals.active_days, 2);
        assert_eq!(month.totals.entries, 3);
    }

    #[test]
    fn month_bounds_handle_december() {
        assert_eq!(
            month_bounds(2026, 12),
            ("2026-12-01".to_string(), "2026-12-31".to_string())
        );
    }

    #[test]
    fn heat_level_thresholds() {
        assert_eq!(heat_level(0), 0);
        assert_eq!(heat_level(1), 1);
        assert_eq!(heat_level(29), 1);
        assert_eq!(heat_level(30), 2);
        assert_eq!(heat_level(60), 3);
        assert_eq!(heat_level(120), 4);
    }
}
