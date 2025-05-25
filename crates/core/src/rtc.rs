use std::mem;
use std::fs::File;
use std::path::Path;
use std::os::unix::io::AsRawFd;
use anyhow::Error;
use nix::{ioctl_read, ioctl_write_ptr, ioctl_none};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use std::collections::BTreeMap;

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

impl RtcTime {
    fn year(&self) -> i32 {
        1900 + self.tm_year as i32
    }
}

impl RtcWkalrm {
    pub fn enabled(&self) -> bool {
        self.enabled == 1
    }

    pub fn year(&self) -> i32 {
        self.time.year()
    }
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

    pub fn set_alarm(&self, duration: Duration) -> Result<i32, Error> {
        let wt = Utc::now() + duration;
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

    pub fn disable_alarm(&self) -> Result<i32, Error> {
        unsafe { rtc_disable_alarm(self.0.as_raw_fd()).map_err(|e| e.into()) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlarmType {
    TrmnlRefresh,
    AutoPowerOff,
}

pub struct ScheduledAlarm {
    pub alarm_type: AlarmType,
    pub wake_time: DateTime<Utc>,
}

pub struct AlarmManager {
    rtc: Rtc,
    scheduled_alarms: BTreeMap<AlarmType, ScheduledAlarm>,
}

impl AlarmManager {
    pub fn new(rtc: Rtc) -> Self {
        AlarmManager {
            rtc,
            scheduled_alarms: BTreeMap::new(),
        }
    }

    pub fn schedule_alarm(
        &mut self,
        alarm_type: AlarmType,
        seconds_from_now: i64,
    ) -> Result<(), Error> {
        let wake_time = Utc::now() + Duration::seconds(seconds_from_now as i64);
        self.scheduled_alarms.insert(
            alarm_type,
            ScheduledAlarm {
                alarm_type,
                wake_time,
            },
        );
        self.update_hardware_alarm()?;
        Ok(())
    }

    pub fn cancel_alarm(&mut self, alarm_type: AlarmType) -> Result<(), Error> {
        self.scheduled_alarms.remove(&alarm_type);
        self.update_hardware_alarm()?;
        Ok(())
    }

    fn update_hardware_alarm(&self) -> Result<(), Error> {
        if let Some((_, earliest_alarm)) = self
            .scheduled_alarms
            .iter()
            .min_by_key(|(_, alarm)| &alarm.wake_time)
        {
            let now = Utc::now();
            let duration = earliest_alarm.wake_time.signed_duration_since(now);
            if duration.num_seconds() > 0 {
                self.rtc.set_alarm(duration)?;
            } else {
                // If the earliest alarm is in the past or now, disable the hardware alarm
                // and let the system wake up naturally or by other means.
                // The check_fired_alarms will handle what needs to be done.
                self.rtc.disable_alarm()?;
            }
        } else {
            self.rtc.disable_alarm()?;
        }
        Ok(())
    }

    pub fn is_alarm_scheduled(&self, alarm_type: AlarmType) -> bool {
        if let Some(scheduled_alarm) = self.scheduled_alarms.get(&alarm_type) {
            scheduled_alarm.wake_time > Utc::now()
        } else {
            false
        }
    }

    pub fn check_fired_alarms(
        &mut self,
        after: DateTime<Utc>,
        before: DateTime<Utc>,
    ) -> Result<Vec<AlarmType>, Error> {
        let mut fired_types = Vec::new();
        let now = Utc::now();

        // Get the earliest scheduled alarm for duration comparison
        if let Some((_, earliest_alarm)) = self
            .scheduled_alarms
            .iter()
            .min_by_key(|(_, alarm)| &alarm.wake_time)
        {
            let expected_duration = earliest_alarm.wake_time.signed_duration_since(now);

            // Check hardware alarm state
            let rwa = self.rtc.alarm()?;
            let hardware_alarm_fired = !rwa.enabled()
                || (rwa.year() <= 1970
                    && ((after - before) - expected_duration).num_seconds().abs() < 3);

            if hardware_alarm_fired {
                // Check which logical alarms should fire
                let mut to_remove = Vec::new();
                for (alarm_type, scheduled_alarm) in &self.scheduled_alarms {
                    if (now - scheduled_alarm.wake_time).abs().num_milliseconds() <= 3000 {
                        fired_types.push(*alarm_type);
                        to_remove.push(*alarm_type);
                    }
                }
                for alarm_type in to_remove {
                    self.scheduled_alarms.remove(&alarm_type);
                }
            }
        }
        self.update_hardware_alarm()?;
        Ok(fired_types)
    }
}
