pub mod alerts;
pub mod realtime;
pub mod static_data;

/// Used to fetch OBA data. Doesn't seem to ever change so its hardcoded
pub const AGENCIES: [&str; 2] = ["MTABC", "MTA NYCT"];
