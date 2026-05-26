use crate::codec::*;

pub fn sr_info_table() -> Vec<SrInfo> {
    vec![
        SrInfo {
            sampling_rate: 96000, num_cb_long: 41, num_cb_short: 12,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,12,12,12,12,12,16,16,24,28,36,44,64,64,64,64,64,64,64,64,64,64,64];
                a[..41].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,8,8,8,16,28,36];
                a[..12].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 88200, num_cb_long: 41, num_cb_short: 12,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,12,12,12,12,12,16,16,24,28,36,44,64,64,64,64,64,64,64,64,64,64,64];
                a[..41].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,8,8,8,16,28,36];
                a[..12].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 64000, num_cb_long: 47, num_cb_short: 12,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,4,4,4,4,8,8,8,8,12,12,12,16,16,16,20,24,24,28,36,40,40,40,40,40,40,40,40,40,40,40,40,40,40,40,40,40,40];
                a[..47].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,8,8,8,16,28,32];
                a[..12].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 48000, num_cb_long: 49, num_cb_short: 14,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,8,8,12,12,12,12,16,16,20,20,24,24,28,28,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,96];
                a[..49].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,8,8,8,12,12,12,16,16,16];
                a[..14].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 44100, num_cb_long: 49, num_cb_short: 14,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,8,8,12,12,12,12,16,16,20,20,24,24,28,28,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,96];
                a[..49].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,8,8,8,12,12,12,16,16,16];
                a[..14].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 32000, num_cb_long: 51, num_cb_short: 14,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,8,8,12,12,12,12,16,16,20,20,24,24,28,28,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32,32];
                a[..51].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,8,8,8,12,12,12,16,16,16];
                a[..14].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 24000, num_cb_long: 47, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,8,8,8,8,8,12,12,12,12,16,16,16,20,20,24,24,28,28,32,36,36,40,44,48,52,52,64,64,64,64,64];
                a[..47].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,8,8,8,12,12,16,16,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 22050, num_cb_long: 47, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [4,4,4,4,4,4,4,4,4,4,4,8,8,8,8,8,8,8,8,8,8,12,12,12,12,16,16,16,20,20,24,24,28,28,32,36,36,40,44,48,52,52,64,64,64,64,64];
                a[..47].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,8,8,8,12,12,16,16,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 16000, num_cb_long: 43, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [8,8,8,8,8,8,8,8,8,8,8,12,12,12,12,12,12,12,12,12,16,16,16,16,20,20,20,24,24,28,28,32,36,40,40,44,48,52,56,60,64,64,64];
                a[..43].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,4,8,8,12,12,16,20,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 12000, num_cb_long: 43, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [8,8,8,8,8,8,8,8,8,8,8,12,12,12,12,12,12,12,12,12,16,16,16,16,20,20,20,24,24,28,28,32,36,40,40,44,48,52,56,60,64,64,64];
                a[..43].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,4,8,8,12,12,16,20,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 11025, num_cb_long: 43, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [8,8,8,8,8,8,8,8,8,8,8,12,12,12,12,12,12,12,12,12,16,16,16,16,20,20,20,24,24,28,28,32,36,40,40,44,48,52,56,60,64,64,64];
                a[..43].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,4,8,8,12,12,16,20,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
        SrInfo {
            sampling_rate: 8000, num_cb_long: 40, num_cb_short: 15,
            cb_width_long: {
                let mut a = [0i32; NSFB_LONG];
                let d = [12,12,12,12,12,12,12,12,12,12,12,12,12,16,16,16,16,16,16,16,20,20,20,20,24,24,24,28,28,32,36,36,40,44,48,52,56,60,64,80];
                a[..40].copy_from_slice(&d);
                a
            },
            cb_width_short: {
                let mut a = [0i32; NSFB_SHORT];
                let d = [4,4,4,4,4,4,4,8,8,8,8,12,16,20,20];
                a[..15].copy_from_slice(&d);
                a
            },
        },
    ]
}
