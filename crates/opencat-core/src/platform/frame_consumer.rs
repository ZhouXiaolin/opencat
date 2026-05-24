use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::media_plan::FrameMediaPlan;

/// Header information passed to frame consumers.
#[derive(Clone, Copy, Debug)]
pub struct RenderSessionHeader {
    pub composition_size: (u32, u32),
    pub fps: u32,
    pub frames: u32,
}

/// A consumer that processes a single rendered frame.
pub trait FrameConsumer {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;

    fn consume_frame(
        &mut self,
        header: &RenderSessionHeader,
        draw: &mut DrawOpFrame,
        plan: &FrameMediaPlan,
    ) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConsumer {
        calls: u32,
    }
    impl FrameConsumer for MockConsumer {
        type Output = u32;
        type Error = std::io::Error;
        fn consume_frame(
            &mut self,
            _header: &RenderSessionHeader,
            _draw: &mut DrawOpFrame,
            _plan: &FrameMediaPlan,
        ) -> Result<u32, Self::Error> {
            self.calls += 1;
            Ok(self.calls)
        }
    }

    #[test]
    fn mock_consumer_round_trip() {
        let mut c = MockConsumer { calls: 0 };
        let header = RenderSessionHeader {
            composition_size: (1920, 1080),
            fps: 30,
            frames: 1,
        };
        let mut draw = DrawOpFrame::default();
        let plan = FrameMediaPlan::default();
        assert_eq!(c.consume_frame(&header, &mut draw, &plan).unwrap(), 1);
        assert_eq!(c.consume_frame(&header, &mut draw, &plan).unwrap(), 2);
    }
}
