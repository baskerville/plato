use std::mem;
use std::fs::File;
use std::path::Path;
use std::os::unix::io::AsRawFd;
use anyhow::Error;
use nix::{ioctl_read, ioctl_write_ptr, ioctl_none};
use chrono::{Duration, Utc, Datelike, Timelike};

ioctl_read!(rtc_read_alarm, b'p', 0x10, RtcWkalrm);
ioctl_write_ptr!(rtc_write_alarm, b'p', 0x0f, RtcWkalrm);
ioctl_none!(rtc_disable_alarm, b'p', 0x02);

#[repr(C)]
#[derive(Debug, Clone)]
pub struct RtcTime {
    tm_sec: libc::c_int,
    tm_min: libc::c_int,
    tm_hour: libc::c_int,
    tm_mday: libc::c_int,
    tm_mon: libc::c_int,
    tm_year: libc::c_int,
    tm_wday: libc::c_int,
    tm_yday: libc::c_int,
    tm_isdst: libc::c_int,
}

impl Default for RtcWkalrm {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct RtcWkalrm {
    enabled: libc::c_uchar,
    pending: libc::c_uchar,
    time: RtcTime,
}

pub struct Rtc(File);

impl Rtc {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Rtc, Error> {
        let file = File::open(path)?;
        Ok(Rtc(file))
    }

    pub fn alarm(&self) -> Result<RtcWkalrm, Error> {
        let mut rwa = RtcWkalrm::default();
        unsafe {
            rtc_read_alarm(self.0.as_raw_fd(), &mut rwa)
                          .map(|_| rwa)
                          .map_err(|e| e.into())
        }
    }

    pub fn set_alarm(&self, days: f32) -> Result<i32, Error> {
        let wt = Utc::now() + Duration::seconds((86_400.0 * days) as i64);
        let rwa = RtcWkalrm {
            enabled: 1,
            pending: 0,
            time: RtcTime {
                tm_sec: wt.second() as libc::c_int,
                tm_min: wt.minute() as libc::c_int,
                tm_hour: wt.hour() as libc::c_int,
                tm_mday: wt.day() as libc::c_int,
                tm_mon: wt.month0() as libc::c_int,
                tm_year: (wt.year() - 1900) as libc::c_int,
                tm_wday: -1,
                tm_yday: -1,
                tm_isdst: -1,
            },
        };
        unsafe { rtc_write_alarm(self.0.as_raw_fd(), &rwa).map_err(|e| e.into()) }
    }

    pub fn is_alarm_enabled(&self) -> Result<bool, Error> {
        self.alarm().map(|rwa| rwa.enabled == 1)
    }

    pub fn disable_alarm(&self) -> Result<i32, Error> {
        unsafe { rtc_disable_alarm(self.0.as_raw_fd()).map_err(|e| e.into()) }
    }
}
