// 1:1 port of faac/libfaac/channels.{h,c}.
//
// Element/Channel layout assignment: the encoder decides which AAC
// syntactic element (SCE/CPE/LFE) each input channel maps to. The mapping
// table at the top of channels.c documents the rules.

use crate::codec::MAX_SCFAC_BANDS;

#[derive(Clone)]
pub struct MsInfo {
    pub is_present: bool,
    pub ms_used: [bool; MAX_SCFAC_BANDS],
}

impl Default for MsInfo {
    fn default() -> Self {
        Self {
            is_present: false,
            ms_used: [false; MAX_SCFAC_BANDS],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum ElementType {
    Sce = 0,
    Cpe = 1,
    Lfe = 2,
}

#[derive(Clone)]
pub struct ChannelInfo {
    pub tag: i32,
    pub present: bool,
    pub ch_is_left: bool,
    pub paired_ch: i32,
    pub common_window: bool,
    pub element_type: ElementType,
    pub ms_info: MsInfo,
}

impl Default for ChannelInfo {
    fn default() -> Self {
        Self {
            tag: 0,
            present: false,
            ch_is_left: false,
            paired_ch: 0,
            common_window: false,
            element_type: ElementType::Sce,
            ms_info: MsInfo::default(),
        }
    }
}

impl ChannelInfo {
    pub fn assign_elements(channel_info: &mut [ChannelInfo], num_channels: i32, use_lfe: bool) {
    let mut sce_tag = 0i32;
    let mut lfe_tag = 0i32;
    let mut cpe_tag = 0i32;
    let mut num_left = num_channels;

    // First element is SCE, except for the pure stereo case.
    if num_left != 2 {
        let idx = (num_channels - num_left) as usize;
        channel_info[idx].present = true;
        channel_info[idx].tag = sce_tag;
        sce_tag += 1;
        channel_info[idx].element_type = ElementType::Sce;
        num_left -= 1;
    }

    // Next elements are CPEs.
    while num_left > 1 {
        // Left channel
        {
            let idx = (num_channels - num_left) as usize;
            channel_info[idx].present = true;
            channel_info[idx].tag = cpe_tag;
            cpe_tag += 1;
            channel_info[idx].common_window = false;
            channel_info[idx].ch_is_left = true;
            channel_info[idx].paired_ch = num_channels - num_left + 1;
            channel_info[idx].element_type = ElementType::Cpe;
            num_left -= 1;
        }
        // Right channel
        {
            let idx = (num_channels - num_left) as usize;
            channel_info[idx].present = true;
            channel_info[idx].common_window = false;
            channel_info[idx].ch_is_left = false;
            channel_info[idx].paired_ch = num_channels - num_left - 1;
            channel_info[idx].element_type = ElementType::Cpe;
            num_left -= 1;
        }
    }

    // Trailing single channel: LFE if requested, otherwise SCE.
    if num_left != 0 {
        let idx = (num_channels - num_left) as usize;
        channel_info[idx].present = true;
        if use_lfe {
            channel_info[idx].tag = lfe_tag;
            lfe_tag += 1;
            channel_info[idx].element_type = ElementType::Lfe;
        } else {
            channel_info[idx].tag = sce_tag;
            sce_tag += 1;
            channel_info[idx].element_type = ElementType::Sce;
        }
    }
    // (sce_tag/lfe_tag/cpe_tag are read-but-discarded after final assignment,
    // matching the C version which has the same "unused increment".)
    let _ = (sce_tag, lfe_tag, cpe_tag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(infos: &[ChannelInfo]) -> Vec<ElementType> {
        infos.iter().map(|c| c.element_type).collect()
    }

    #[test]
    fn mono_is_one_sce() {
        let mut ci = vec![ChannelInfo::default(); 1];
        ChannelInfo::assign_elements(&mut ci, 1, false);
        assert_eq!(types(&ci), vec![ElementType::Sce]);
        assert!(ci[0].present);
        assert_eq!(ci[0].tag, 0);
    }

    #[test]
    fn stereo_is_one_cpe() {
        let mut ci = vec![ChannelInfo::default(); 2];
        ChannelInfo::assign_elements(&mut ci, 2, false);
        assert_eq!(types(&ci), vec![ElementType::Cpe, ElementType::Cpe]);
        assert!(ci[0].ch_is_left);
        assert_eq!(ci[0].paired_ch, 1);
        assert!(!ci[1].ch_is_left);
        assert_eq!(ci[1].paired_ch, 0);
    }

    #[test]
    fn five_one_with_lfe() {
        // 6 channels with LFE = 1 SCE + 2 CPE + 1 LFE (per table comment in channels.c)
        let mut ci = vec![ChannelInfo::default(); 6];
        ChannelInfo::assign_elements(&mut ci, 6, true);
        assert_eq!(
            types(&ci),
            vec![
                ElementType::Sce,
                ElementType::Cpe,
                ElementType::Cpe,
                ElementType::Cpe,
                ElementType::Cpe,
                ElementType::Lfe,
            ]
        );
    }
}
