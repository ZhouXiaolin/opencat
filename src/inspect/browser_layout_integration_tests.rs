//! Integration tests for browser layout parity.
//!
//! These fixtures verify that **combinations** of Tailwind utility classes produce
//! identical layouts in Chrome and Taffy. They are not covered by the auto-generated
//! unit tests in `GENERATED_LAYOUT_GROUP_SPECS`, which only test a single utility
//! class in isolation.
//!
//! Each fixture represents a real-world UI pattern (card, nav bar, sidebar, form,
//! etc.) and exercises the interaction of multiple CSS properties together.
//!
//! # Why These Tests Are Manual
//!
//! Auto-generating all possible utility combinations would create thousands of tests
//! with diminishing returns. These curated fixtures cover common UI patterns that:
//! - Test multiple utilities working together
//! - Exercise edge cases (text wrapping, nested flex, absolute positioning)
//! - Represent real-world layouts developers actually build
//!
//! # Maintenance
//!
//! When adding new integration fixtures:
//! 1. Ensure the scenario tests a meaningful combination of utilities
//! 2. Avoid duplicating patterns already covered by other fixtures
//! 3. Use realistic viewport dimensions and tolerance values

use super::browser_layout_tests::{FixtureNode, LayoutFixture};

// ---------------------------------------------------------------------------
// Integration fixture definitions
// ---------------------------------------------------------------------------

fn browser_layout_integration_fixtures() -> Vec<LayoutFixture> {
    vec![
        // ── Block flow ───────────────────────────────────────────────────
        LayoutFixture {
            name: "block-flow-stacks-siblings",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![
                    FixtureNode::div(
                        "header",
                        "pt-[20px] pb-[20px]",
                        vec![FixtureNode::text("header-text", "text-[24px]", "Header")],
                    ),
                    FixtureNode::div(
                        "content",
                        "pt-[10px] pb-[10px]",
                        vec![FixtureNode::text("content-text", "text-[18px]", "Content")],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "block-flow-varied-widths",
            viewport_width: 320,
            viewport_height: 200,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[16px]",
                vec![
                    FixtureNode::div("full-row", "w-full h-[32px] mb-[8px]", vec![]),
                    FixtureNode::div("half-row", "w-[140px] h-[24px] mb-[8px]", vec![]),
                    FixtureNode::div("third-row", "w-[96px] h-[20px] mb-[8px]", vec![]),
                    FixtureNode::div("wide-row", "w-[280px] h-[28px]", vec![]),
                ],
            ),
        },
        // ── Flex row – justify / align ──────────────────────────────────
        LayoutFixture {
            name: "flex-row-justify-between",
            viewport_width: 390,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-between items-center w-full h-full px-[24px] py-[16px]",
                vec![
                    FixtureNode::div("left", "w-[56px] h-[56px]", vec![]),
                    FixtureNode::div("center", "w-[72px] h-[40px]", vec![]),
                    FixtureNode::div("right", "w-[56px] h-[56px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "justify-around-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-around items-center w-full h-full",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "justify-start-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-start items-center w-full h-full gap-[12px] px-[16px]",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "justify-evenly-four-pills",
            viewport_width: 420,
            viewport_height: 96,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-evenly items-center w-full h-full",
                vec![
                    FixtureNode::div("pill-a", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-b", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-c", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-d", "w-[56px] h-[24px]", vec![]),
                ],
            ),
        },
        // ── Flex row – items alignment ───────────────────────────────────
        LayoutFixture {
            name: "items-start-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-start w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-center-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-end-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-end w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-stretch-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-stretch w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px]", vec![]),
                ],
            ),
        },
        // ── Flex column ─────────────────────────────────────────────────
        LayoutFixture {
            name: "flex-col-gap-padding",
            viewport_width: 280,
            viewport_height: 200,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col gap-[12px] w-[180px] h-[120px] px-[16px] py-[12px]",
                    vec![
                        FixtureNode::div("title", "w-[90px] h-[20px]", vec![]),
                        FixtureNode::div("body", "w-[140px] h-[32px]", vec![]),
                        FixtureNode::div("footer", "w-[60px] h-[16px] mt-[4px]", vec![]),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "flex-column-children-stretch",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-[320px] h-[180px]",
                vec![FixtureNode::div("header", "h-[40px]", vec![])],
            ),
        },
        LayoutFixture {
            name: "justify-between-column-stretch",
            viewport_width: 320,
            viewport_height: 240,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col justify-between w-[280px] h-[220px] p-[16px]",
                vec![
                    FixtureNode::div("top", "w-full h-[40px]", vec![]),
                    FixtureNode::div("mid", "w-full h-[40px]", vec![]),
                    FixtureNode::div("bottom", "w-full h-[40px]", vec![]),
                ],
            ),
        },
        // ── Text wrapping ────────────────────────────────────────────────
        LayoutFixture {
            name: "text-wraps-within-parent-card-width",
            viewport_width: 220,
            viewport_height: 180,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "card",
                    "w-[160px] px-[8px] py-[8px]",
                    vec![FixtureNode::text(
                        "body",
                        "text-[16px]",
                        "从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问。",
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "stretched-flex-column-card-wraps-text",
            viewport_width: 520,
            viewport_height: 220,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col w-[440px] border-2 border-blue-200",
                    vec![FixtureNode::div(
                        "card-body",
                        "flex flex-col gap-[16px] p-[20px]",
                        vec![FixtureNode::text(
                            "card-text",
                            "text-[15px] text-slate-600 leading-relaxed",
                            "从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问，每一次实验都是新的发现！",
                        )],
                    )],
                )],
            ),
        },
        // ── Absolute positioning ────────────────────────────────────────
        LayoutFixture {
            name: "absolute-inset-layout",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div(
                        "badge",
                        "absolute left-[12px] top-[10px] w-[80px] h-[24px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "panel",
                        "absolute right-[18px] bottom-[16px] w-[120px] h-[64px]",
                        vec![],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "absolute-corners-badges",
            viewport_width: 300,
            viewport_height: 160,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div(
                        "top-left",
                        "absolute left-[8px] top-[8px] w-[36px] h-[20px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "top-right",
                        "absolute right-[8px] top-[8px] w-[36px] h-[20px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "bottom-left",
                        "absolute left-[8px] bottom-[8px] w-[36px] h-[20px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "bottom-right",
                        "absolute right-[8px] bottom-[8px] w-[36px] h-[20px]",
                        vec![],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "nested-absolute-in-absolute",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![FixtureNode::div(
                    "panel",
                    "absolute left-[20px] top-[20px] w-[200px] h-[120px]",
                    vec![FixtureNode::div(
                        "inner",
                        "absolute right-[8px] bottom-[8px] w-[60px] h-[30px]",
                        vec![],
                    )],
                )],
            ),
        },
        // ── Auto-sized flex column labels (nav grid) ────────────────────
        LayoutFixture {
            name: "auto-sized-flex-column-labels",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-between w-full px-[20px] py-[16px]",
                vec![
                    FixtureNode::div(
                        "cat-pizza",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-pizza-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text("cat-pizza-text", "text-[12px] font-medium", "Pizza"),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-burger",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-burger-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text(
                                "cat-burger-text",
                                "text-[12px] font-medium",
                                "Burger",
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-sushi",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-sushi-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text("cat-sushi-text", "text-[12px] font-medium", "Sushi"),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-salad",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-salad-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text("cat-salad-text", "text-[12px] font-medium", "Salad"),
                        ],
                    ),
                ],
            ),
        },
        // ── Promo banner patterns ────────────────────────────────────────
        LayoutFixture {
            name: "auto-sized-flex-column-prefers-single-line",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "promo-banner",
                    "flex flex-row items-center w-[350px] px-[20px] py-[16px]",
                    vec![FixtureNode::div(
                        "promo-text",
                        "flex flex-col gap-[4px]",
                        vec![
                            FixtureNode::text("promo-title", "text-[18px] font-bold", "50% OFF"),
                            FixtureNode::text("promo-desc", "text-[13px]", "First order discount"),
                        ],
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "fixed-width-flex-column-text-wraps",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "promo-text",
                    "flex flex-col w-[80px] gap-[4px]",
                    vec![
                        FixtureNode::text("promo-title", "text-[18px] font-bold", "50% OFF"),
                        FixtureNode::text("promo-desc", "text-[13px]", "First order discount"),
                    ],
                )],
            ),
        },
        // ── Nested full-width shell ──────────────────────────────────────
        LayoutFixture {
            name: "nested-full-width-shell",
            viewport_width: 420,
            viewport_height: 220,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "shell",
                    "flex flex-col w-full h-full px-[12px] py-[10px]",
                    vec![
                        FixtureNode::div("header", "w-full h-[32px]", vec![]),
                        FixtureNode::div(
                            "content",
                            "flex flex-row justify-between w-full mt-[12px]",
                            vec![
                                FixtureNode::div("content-left", "w-[120px] h-[96px]", vec![]),
                                FixtureNode::div("content-right", "w-[180px] h-[96px]", vec![]),
                            ],
                        ),
                    ],
                )],
            ),
        },
        // ── Status bar single-line ───────────────────────────────────────
        LayoutFixture {
            name: "fixed-width-flex-row-text-stays-single-line",
            viewport_width: 390,
            viewport_height: 120,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "status-bar",
                    "flex flex-row justify-between items-center w-full h-[44px] px-[24px]",
                    vec![FixtureNode::text(
                        "status-time",
                        "text-[15px] font-semibold",
                        "9:41",
                    )],
                )],
            ),
        },
        // ── Row gap with margins ─────────────────────────────────────────
        LayoutFixture {
            name: "row-gap-with-margins",
            viewport_width: 360,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("banner", "w-full h-[40px] mb-[8px]", vec![]),
                    FixtureNode::div(
                        "actions",
                        "flex flex-row gap-[12px] mt-[4px]",
                        vec![
                            FixtureNode::div("action-a", "w-[80px] h-[32px]", vec![]),
                            FixtureNode::div("action-b", "w-[80px] h-[32px]", vec![]),
                            FixtureNode::div("action-c", "w-[80px] h-[32px]", vec![]),
                        ],
                    ),
                ],
            ),
        },
        // ── Spacing scale (Tailwind numeric scale) ──────────────────────
        LayoutFixture {
            name: "spacing-scale-padding-card",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-4",
                vec![FixtureNode::div(
                    "card",
                    "w-32 h-20 px-4 py-2",
                    vec![
                        FixtureNode::div("card-title", "w-16 h-4 mb-2", vec![]),
                        FixtureNode::div("card-body", "w-20 h-8", vec![]),
                    ],
                )],
            ),
        },
        // ── Flex grow / basis / shrink ───────────────────────────────────
        LayoutFixture {
            name: "grow-basis-row",
            viewport_width: 420,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[12px] px-[16px]",
                vec![
                    FixtureNode::div("fixed", "w-[64px] h-[36px]", vec![]),
                    FixtureNode::div("grow-a", "basis-[80px] grow h-[36px]", vec![]),
                    FixtureNode::div("grow-b", "basis-20 grow-2 h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "shrink-constrained-row",
            viewport_width: 260,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[8px] px-[12px]",
                vec![
                    FixtureNode::div("left", "w-[96px] h-[28px] shrink-0", vec![]),
                    FixtureNode::div("mid", "w-[96px] h-[28px] shrink", vec![]),
                    FixtureNode::div("right", "w-[96px] h-[28px] shrink", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "basis-mixed-row",
            viewport_width: 480,
            viewport_height: 128,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[10px] px-[16px]",
                vec![
                    FixtureNode::div("basis-a", "basis-16 h-[32px]", vec![]),
                    FixtureNode::div("basis-b", "basis-24 h-[32px]", vec![]),
                    FixtureNode::div("basis-c", "basis-[140px] h-[32px]", vec![]),
                ],
            ),
        },
        // ── Padding / margin on both axes ────────────────────────────────
        LayoutFixture {
            name: "padding-sides-shell",
            viewport_width: 360,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-4",
                vec![FixtureNode::div(
                    "panel",
                    "w-40 h-24 pt-4 pr-6 pb-8 pl-2",
                    vec![
                        FixtureNode::div("panel-title", "w-16 h-4", vec![]),
                        FixtureNode::div("panel-copy", "w-20 h-4 mt-2", vec![]),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "margin-axis-flex-row",
            viewport_width: 360,
            viewport_height: 160,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-start w-full h-full px-4 py-4",
                vec![
                    FixtureNode::div("chip-a", "w-12 h-12 ml-4 mr-2", vec![]),
                    FixtureNode::div("chip-b", "w-12 h-12 mx-3 mt-4", vec![]),
                    FixtureNode::div("chip-c", "w-12 h-12 mr-4 mb-2", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "margin-axis-flex-col",
            viewport_width: 240,
            viewport_height: 220,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col items-start w-full h-full px-4 py-4",
                vec![
                    FixtureNode::div("row-a", "w-20 h-6 mt-4 mb-2", vec![]),
                    FixtureNode::div("row-b", "w-24 h-6 my-3", vec![]),
                    FixtureNode::div("row-c", "w-16 h-6 ml-4", vec![]),
                ],
            ),
        },
        // ── Inset axis matrix ────────────────────────────────────────────
        LayoutFixture {
            name: "inset-axis-matrix",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div("top-bar", "absolute inset-x-4 top-[12px] h-[20px]", vec![]),
                    FixtureNode::div(
                        "left-rail",
                        "absolute left-[10px] inset-y-4 w-[24px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "bottom-bar",
                        "absolute inset-x-[20px] bottom-[18px] h-[16px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "right-badge",
                        "absolute right-[14px] top-[48px] w-[48px] h-[24px]",
                        vec![],
                    ),
                ],
            ),
        },
        // ── Text tracking / leading ──────────────────────────────────────
        LayoutFixture {
            name: "tracking-preset-stack",
            viewport_width: 420,
            viewport_height: 180,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full gap-2 p-4",
                vec![
                    FixtureNode::text("track-tight", "text-[16px] tracking-tight", "Tailwind"),
                    FixtureNode::text("track-normal", "text-[16px] tracking-normal", "Tailwind"),
                    FixtureNode::text("track-wide", "text-[16px] tracking-wide", "Tailwind"),
                    FixtureNode::text(
                        "track-wider",
                        "text-[16px] tracking-wider uppercase",
                        "Tailwind",
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "text-size-extended-stack",
            viewport_width: 320,
            viewport_height: 260,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full gap-[10px] p-[20px]",
                vec![
                    FixtureNode::text("txt-xs", "text-xs", "Scale"),
                    FixtureNode::text("txt-sm", "text-sm", "Scale"),
                    FixtureNode::text("txt-base", "text-base", "Scale"),
                    FixtureNode::text("txt-lg", "text-lg", "Scale"),
                    FixtureNode::text("txt-xl", "text-xl", "Scale"),
                    FixtureNode::text("txt-2xl", "text-2xl", "Scale"),
                ],
            ),
        },
        LayoutFixture {
            name: "text-leading-and-tracking-stack",
            viewport_width: 340,
            viewport_height: 240,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![
                    FixtureNode::div(
                        "tight-wrap",
                        "mb-[12px]",
                        vec![FixtureNode::text(
                            "lead-tight",
                            "text-[16px] leading-[18px]",
                            "Tight leading",
                        )],
                    ),
                    FixtureNode::div(
                        "relaxed-wrap",
                        "mb-[12px]",
                        vec![FixtureNode::text(
                            "lead-relaxed",
                            "text-[16px] leading-relaxed",
                            "Relaxed leading",
                        )],
                    ),
                    FixtureNode::div(
                        "tracking-wrap",
                        "",
                        vec![FixtureNode::text(
                            "track-wide",
                            "text-[16px] tracking-[1.5px] uppercase",
                            "Wide tracking",
                        )],
                    ),
                ],
            ),
        },
        // ── Deep nested flex ─────────────────────────────────────────────
        LayoutFixture {
            name: "deep-nested-card",
            viewport_width: 360,
            viewport_height: 260,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-4",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col w-[280px] gap-[12px] p-[16px]",
                    vec![
                        FixtureNode::div(
                            "header-row",
                            "flex flex-row items-center justify-between",
                            vec![
                                FixtureNode::div("avatar", "w-[32px] h-[32px]", vec![]),
                                FixtureNode::div(
                                    "header-text",
                                    "flex flex-col gap-[2px] ml-[8px]",
                                    vec![
                                        FixtureNode::text("name", "text-[14px] font-bold", "Alice"),
                                        FixtureNode::text("time", "text-[11px]", "2 min ago"),
                                    ],
                                ),
                                FixtureNode::div("badge", "w-[40px] h-[20px]", vec![]),
                            ],
                        ),
                        FixtureNode::div("divider", "w-full h-[1px]", vec![]),
                        FixtureNode::text(
                            "body-text",
                            "text-[13px]",
                            "Nested flex layout with header row, body and footer.",
                        ),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "three-level-nested-flex",
            viewport_width: 400,
            viewport_height: 280,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full p-[16px] gap-[12px]",
                vec![
                    FixtureNode::div(
                        "nav",
                        "flex flex-row justify-between w-full h-[40px] px-[12px]",
                        vec![
                            FixtureNode::div(
                                "nav-left",
                                "flex flex-row items-center gap-[8px]",
                                vec![
                                    FixtureNode::div("logo", "w-[24px] h-[24px]", vec![]),
                                    FixtureNode::text("brand", "text-[16px] font-bold", "Brand"),
                                ],
                            ),
                            FixtureNode::div(
                                "nav-right",
                                "flex flex-row items-center gap-[12px]",
                                vec![
                                    FixtureNode::div("icon-a", "w-[20px] h-[20px]", vec![]),
                                    FixtureNode::div("icon-b", "w-[20px] h-[20px]", vec![]),
                                ],
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "content",
                        "flex flex-row w-full gap-[12px]",
                        vec![
                            FixtureNode::div(
                                "sidebar",
                                "flex flex-col gap-[8px] w-[80px]",
                                vec![
                                    FixtureNode::div("sb-item-1", "w-full h-[28px]", vec![]),
                                    FixtureNode::div("sb-item-2", "w-full h-[28px]", vec![]),
                                    FixtureNode::div("sb-item-3", "w-full h-[28px]", vec![]),
                                ],
                            ),
                            FixtureNode::div(
                                "main",
                                "flex flex-col gap-[10px] grow",
                                vec![
                                    FixtureNode::div("card-a", "w-full h-[48px]", vec![]),
                                    FixtureNode::div("card-b", "w-full h-[48px]", vec![]),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
        },
        // ── Equal-grow columns ───────────────────────────────────────────
        LayoutFixture {
            name: "equal-grow-four-columns",
            viewport_width: 400,
            viewport_height: 100,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row w-full h-full gap-[6px] px-[10px]",
                vec![
                    FixtureNode::div("col-a", "grow h-full", vec![]),
                    FixtureNode::div("col-b", "grow h-full", vec![]),
                    FixtureNode::div("col-c", "grow h-full", vec![]),
                    FixtureNode::div("col-d", "grow h-full", vec![]),
                ],
            ),
        },
        // ── Absolute within flex ─────────────────────────────────────────
        LayoutFixture {
            name: "absolute-badge-in-flex-row",
            viewport_width: 320,
            viewport_height: 100,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full px-[16px] gap-[12px]",
                vec![
                    FixtureNode::div(
                        "icon-wrap",
                        "relative w-[48px] h-[48px]",
                        vec![
                            FixtureNode::div("icon", "w-[48px] h-[48px]", vec![]),
                            FixtureNode::div(
                                "dot",
                                "absolute right-0 top-0 w-[10px] h-[10px]",
                                vec![],
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "text-col",
                        "flex flex-col gap-[4px]",
                        vec![
                            FixtureNode::text("title", "text-[14px] font-bold", "Notification"),
                            FixtureNode::text("desc", "text-[12px]", "You have a new message"),
                        ],
                    ),
                ],
            ),
        },
        // ── Centered card in viewport ────────────────────────────────────
        LayoutFixture {
            name: "centered-card-viewport",
            viewport_width: 390,
            viewport_height: 220,
            tolerance_px: 10.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col items-center justify-center w-full h-full",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col items-center gap-[12px] w-[280px] px-[20px] py-[24px]",
                    vec![
                        FixtureNode::div("icon", "w-[48px] h-[48px]", vec![]),
                        FixtureNode::text("heading", "text-[18px] font-bold", "Welcome"),
                        FixtureNode::text("sub", "text-[13px]", "Get started with CatCut"),
                    ],
                )],
            ),
        },
        // ── Sidebar + main layout ────────────────────────────────────────
        LayoutFixture {
            name: "sidebar-main-layout",
            viewport_width: 480,
            viewport_height: 240,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row w-full h-full",
                vec![
                    FixtureNode::div(
                        "sidebar",
                        "flex flex-col gap-[12px] w-[100px] p-[12px]",
                        vec![
                            FixtureNode::div("nav-a", "w-full h-[28px]", vec![]),
                            FixtureNode::div("nav-b", "w-full h-[28px]", vec![]),
                            FixtureNode::div("nav-c", "w-full h-[28px]", vec![]),
                        ],
                    ),
                    FixtureNode::div(
                        "main",
                        "flex flex-col gap-[12px] grow p-[16px]",
                        vec![
                            FixtureNode::div("card-top", "w-full h-[60px]", vec![]),
                            FixtureNode::div("card-bot", "w-full h-[60px]", vec![]),
                        ],
                    ),
                ],
            ),
        },
        // ── Text with font-weight in card ────────────────────────────────
        LayoutFixture {
            name: "text-font-weight-in-card",
            viewport_width: 320,
            viewport_height: 200,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[16px]",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col gap-[8px] w-[240px] p-[16px]",
                    vec![
                        FixtureNode::text("title-bold", "text-[16px] font-bold", "Bold Title"),
                        FixtureNode::text(
                            "sub-medium",
                            "text-[14px] font-medium",
                            "Medium Subtitle",
                        ),
                        FixtureNode::text(
                            "body-normal",
                            "text-[13px]",
                            "Normal body text goes here.",
                        ),
                        FixtureNode::text("footer-light", "text-[11px]", "Light footer"),
                    ],
                )],
            ),
        },
        // ── Flex row with text auto-sizing and fixed box ─────────────────
        LayoutFixture {
            name: "flex-row-text-icon-row",
            viewport_width: 360,
            viewport_height: 80,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full px-[16px] gap-[12px]",
                vec![
                    FixtureNode::div("icon", "w-[32px] h-[32px] shrink-0", vec![]),
                    FixtureNode::text("label", "text-[14px]", "Menu Item"),
                    FixtureNode::div("spacer", "grow h-[1px]", vec![]),
                    FixtureNode::div("chevron", "w-[16px] h-[16px] shrink-0", vec![]),
                ],
            ),
        },
        // ── Absolute overlay on flex card ────────────────────────────────
        LayoutFixture {
            name: "absolute-overlay-on-flex-card",
            viewport_width: 360,
            viewport_height: 160,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[16px]",
                vec![FixtureNode::div(
                    "card",
                    "relative flex flex-col gap-[8px] w-[280px] p-[16px]",
                    vec![
                        FixtureNode::text("title", "text-[16px] font-bold", "Card Title"),
                        FixtureNode::text("body", "text-[13px]", "Card body text."),
                        FixtureNode::div(
                            "badge",
                            "absolute right-[8px] top-[8px] w-[48px] h-[20px]",
                            vec![],
                        ),
                    ],
                )],
            ),
        },
        // ── Mixed grow and fixed width ───────────────────────────────────
        LayoutFixture {
            name: "mixed-grow-fixed-row",
            viewport_width: 400,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[8px] px-[12px]",
                vec![
                    FixtureNode::div("fixed-left", "w-[48px] h-[36px] shrink-0", vec![]),
                    FixtureNode::div("grow-a", "grow h-[36px]", vec![]),
                    FixtureNode::div("fixed-mid", "w-[64px] h-[36px] shrink-0", vec![]),
                    FixtureNode::div("grow-b", "grow h-[36px]", vec![]),
                    FixtureNode::div("fixed-right", "w-[48px] h-[36px] shrink-0", vec![]),
                ],
            ),
        },
        // ── Tab bar pattern ──────────────────────────────────────────────
        LayoutFixture {
            name: "tab-bar-pattern",
            viewport_width: 390,
            viewport_height: 60,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-around items-center w-full h-full px-[8px]",
                vec![
                    FixtureNode::div(
                        "tab-a",
                        "flex flex-col items-center gap-[2px]",
                        vec![
                            FixtureNode::div("tab-a-icon", "w-[20px] h-[20px]", vec![]),
                            FixtureNode::text("tab-a-label", "text-[10px]", "Home"),
                        ],
                    ),
                    FixtureNode::div(
                        "tab-b",
                        "flex flex-col items-center gap-[2px]",
                        vec![
                            FixtureNode::div("tab-b-icon", "w-[20px] h-[20px]", vec![]),
                            FixtureNode::text("tab-b-label", "text-[10px]", "Search"),
                        ],
                    ),
                    FixtureNode::div(
                        "tab-c",
                        "flex flex-col items-center gap-[2px]",
                        vec![
                            FixtureNode::div("tab-c-icon", "w-[20px] h-[20px]", vec![]),
                            FixtureNode::text("tab-c-label", "text-[10px]", "Profile"),
                        ],
                    ),
                    FixtureNode::div(
                        "tab-d",
                        "flex flex-col items-center gap-[2px]",
                        vec![
                            FixtureNode::div("tab-d-icon", "w-[20px] h-[20px]", vec![]),
                            FixtureNode::text("tab-d-label", "text-[10px]", "Settings"),
                        ],
                    ),
                ],
            ),
        },
        // ── Horizontal scroll-like row ───────────────────────────────────
        LayoutFixture {
            name: "horizontal-scroll-row",
            viewport_width: 390,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full py-[12px]",
                vec![
                    FixtureNode::text("section-title", "text-[14px] font-bold mb-[8px]", "Section"),
                    FixtureNode::div(
                        "scroll-row",
                        "flex flex-row gap-[10px] px-[16px]",
                        vec![
                            FixtureNode::div("card-a", "w-[120px] h-[64px] shrink-0", vec![]),
                            FixtureNode::div("card-b", "w-[120px] h-[64px] shrink-0", vec![]),
                            FixtureNode::div("card-c", "w-[120px] h-[64px] shrink-0", vec![]),
                            FixtureNode::div("card-d", "w-[120px] h-[64px] shrink-0", vec![]),
                        ],
                    ),
                ],
            ),
        },
        // ── Form-like column ─────────────────────────────────────────────
        LayoutFixture {
            name: "form-like-column",
            viewport_width: 320,
            viewport_height: 260,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full p-[20px] gap-[16px]",
                vec![
                    FixtureNode::text("form-title", "text-[18px] font-bold", "Sign In"),
                    FixtureNode::div(
                        "field-1",
                        "flex flex-col gap-[4px]",
                        vec![
                            FixtureNode::text("label-1", "text-[12px]", "Email"),
                            FixtureNode::div("input-1", "w-full h-[36px]", vec![]),
                        ],
                    ),
                    FixtureNode::div(
                        "field-2",
                        "flex flex-col gap-[4px]",
                        vec![
                            FixtureNode::text("label-2", "text-[12px]", "Password"),
                            FixtureNode::div("input-2", "w-full h-[36px]", vec![]),
                        ],
                    ),
                    FixtureNode::div("submit-btn", "w-full h-[40px] mt-[8px]", vec![]),
                ],
            ),
        },
        // ── Large gap flex row ───────────────────────────────────────────
        LayoutFixture {
            name: "large-gap-flex-row",
            viewport_width: 400,
            viewport_height: 80,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center justify-center w-full h-full gap-[32px]",
                vec![
                    FixtureNode::div("dot-a", "w-[16px] h-[16px]", vec![]),
                    FixtureNode::div("dot-b", "w-[16px] h-[16px]", vec![]),
                    FixtureNode::div("dot-c", "w-[16px] h-[16px]", vec![]),
                ],
            ),
        },
    ]
}
