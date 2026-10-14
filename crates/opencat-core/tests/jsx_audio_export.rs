//! `export const audio = [...]` — a global audio POOL, next to `meta` / `fonts`
//! like a font manifest. Audio has no visual box, so it is not a scene and
//! cannot sit inside a `<tl>`; it is attached where it is USED, via an
//! `audioId` prop on the node that plays it (a `<tl>`, or a scene).

use opencat_core::parse::jsx::{audio_lines_from_extract, from_extract};
use serde_json::{Value, json};

fn extract(audio: Value, audio_sites: Value) -> Value {
    json!({ "meta": {}, "fonts": null, "audio": audio, "audioSites": audio_sites })
}

#[test]
fn pooled_audio_attaches_where_audio_id_is_written() {
    let value = extract(
        json!([
            { "id": "bgm", "path": "assets/bgm.mp3" },
            { "id": "hit", "url": "https://example.com/hit.mp3", "duration": 0.5 },
        ]),
        json!([
            { "nodeId": "main-tl", "audioId": "bgm" },
            { "nodeId": "scene-01", "audioId": "hit" },
        ]),
    );

    let lines = audio_lines_from_extract(&value).unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["type"], "audio");
    assert_eq!(lines[0]["id"], "bgm");
    assert_eq!(lines[0]["attach"], "main-tl");
    assert_eq!(lines[0]["path"], "assets/bgm.mp3");
    assert!(lines[0].get("url").is_none());

    assert_eq!(lines[1]["id"], "hit");
    assert_eq!(lines[1]["url"], "https://example.com/hit.mp3");
    assert_eq!(lines[1]["attach"], "scene-01");
    assert_eq!(lines[1]["duration"], 0.5);
}

#[test]
fn declared_but_unused_pool_source_emits_no_line() {
    let value = extract(
        json!([{ "id": "bgm", "path": "assets/bgm.mp3" }]),
        json!([]),
    );
    assert!(audio_lines_from_extract(&value).unwrap().is_empty());
}

#[test]
fn absent_audio_export_yields_no_lines() {
    let value = json!({ "meta": {}, "fonts": null });
    assert!(audio_lines_from_extract(&value).unwrap().is_empty());
}

#[test]
fn audio_id_must_reference_a_pooled_source() {
    let value = extract(json!([{ "id": "bgm", "path": "a.mp3" }]), json!([{ "nodeId": "main-tl", "audioId": "nope" }]));
    let err = audio_lines_from_extract(&value).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("audioId `nope`"), "{text}");
    assert!(text.contains("is not declared in `export const audio`"), "{text}");
    assert!(text.contains("on `main-tl`"), "{text}");
}

#[test]
fn audio_pool_source_is_exactly_one_of_path_or_url() {
    let both = extract(
        json!([{ "id": "a", "path": "a.mp3", "url": "https://example.com/a.mp3" }]),
        json!([{ "nodeId": "main-tl", "audioId": "a" }]),
    );
    let err = audio_lines_from_extract(&both).unwrap_err();
    assert!(err.to_string().contains("only one of path / url"), "{err}");

    let neither = extract(json!([{ "id": "a" }]), json!([{ "nodeId": "main-tl", "audioId": "a" }]));
    let err = audio_lines_from_extract(&neither).unwrap_err();
    assert!(err.to_string().contains("needs path or url"), "{err}");
}

#[test]
fn audio_pool_entry_requires_a_non_empty_id() {
    let value = extract(json!([{ "path": "a.mp3" }]), json!([]));
    let err = audio_lines_from_extract(&value).unwrap_err();
    assert!(err.to_string().contains("missing `id`"), "{err}");
}

#[test]
fn audio_pool_duration_must_be_positive() {
    let value = extract(
        json!([{ "id": "a", "path": "a.mp3", "duration": 0 }]),
        json!([{ "nodeId": "main-tl", "audioId": "a" }]),
    );
    let err = audio_lines_from_extract(&value).unwrap_err();
    assert!(err.to_string().contains("must be > 0"), "{err}");
}

#[test]
fn audio_pool_must_be_an_array() {
    let value = extract(json!({ "id": "a" }), json!([]));
    let err = audio_lines_from_extract(&value).unwrap_err();
    assert!(err.to_string().contains("must be an array"), "{err}");
}

/// A pooled source attached to a `<tl>` id and to a scene id both survive the
/// shared assembly, i.e. they become real (resolved) audio sources.
#[test]
fn pooled_audio_survives_the_shared_assembly() {
    let extract_value = json!({
        "meta": { "width": 640, "height": 360, "fps": 30, "duration": 2.0 },
        "audio": [
            { "id": "bgm", "path": "assets/bgm.mp3" },
            { "id": "hit", "url": "https://example.com/hit.mp3", "duration": 0.5 },
        ],
        "audioSites": [
            { "nodeId": "main-tl", "audioId": "bgm" },
            { "nodeId": "scene-a", "audioId": "hit" },
        ],
        "lines": [
            { "type": "div", "id": "root", "parentId": null },
            { "type": "tl", "id": "main-tl", "parentId": "root" },
            { "type": "div", "id": "scene-a", "parentId": "main-tl", "duration": 2.0 },
            { "type": "transition", "id": "t1", "parentId": "main-tl", "from": "scene-a", "to": "scene-b", "effect": "fade", "duration": 0.5 },
            { "type": "div", "id": "scene-b", "parentId": "main-tl", "duration": 2.0 },
        ],
    });

    let parsed = from_extract(&extract_value, "", None).unwrap();

    let sources = &parsed.audio_sources;
    assert_eq!(sources.len(), 2);
    let bgm = sources.iter().find(|s| s.id == "bgm").expect("bgm");
    assert_eq!(bgm.attach, opencat_core::parse::composition::AudioAttachment::Timeline);
    let hit = sources.iter().find(|s| s.id == "hit").expect("hit");
    assert_eq!(
        hit.attach,
        opencat_core::parse::composition::AudioAttachment::Scene { scene_id: "scene-a".to_string() }
    );
    assert_eq!(hit.duration_secs, Some(0.5));
}

/// `audioId` on a node that is neither the `<tl>` nor one of its scenes is a
/// dangling attachment and must fail, same as an xml `<audio attach="...">`.
#[test]
fn pooling_attachment_is_validated_against_real_nodes() {
    let extract_value = json!({
        "meta": { "width": 640, "height": 360, "fps": 30, "duration": 2.0 },
        "audio": [{ "id": "bgm", "path": "assets/bgm.mp3" }],
        "audioSites": [{ "nodeId": "not-a-node", "audioId": "bgm" }],
        "lines": [
            { "type": "div", "id": "root", "parentId": null },
            { "type": "tl", "id": "main-tl", "parentId": "root" },
            { "type": "div", "id": "scene-a", "parentId": "main-tl", "duration": 2.0 },
            { "type": "transition", "id": "t1", "parentId": "main-tl", "from": "scene-a", "to": "scene-b", "effect": "fade", "duration": 0.5 },
            { "type": "div", "id": "scene-b", "parentId": "main-tl", "duration": 2.0 },
        ],
    });

    let err = from_extract(&extract_value, "", None).unwrap_err();
    assert!(
        err.to_string().contains("audio `bgm` attach references non-existent element `not-a-node`"),
        "{err}"
    );
}

/// One pooled source used by two nodes plays twice (two attachment sites).
#[test]
fn one_pooled_source_can_be_attached_in_several_places() {
    let value = extract(
        json!([{ "id": "hit", "path": "a.mp3" }]),
        json!([
            { "nodeId": "scene-a", "audioId": "hit" },
            { "nodeId": "scene-b", "audioId": "hit" },
        ]),
    );
    let lines = audio_lines_from_extract(&value).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["attach"], "scene-a");
    assert_eq!(lines[1]["attach"], "scene-b");
}

/// `audioSites` absent (no `audioId` anywhere) is fine — no audio at all.
#[test]
fn no_sites_and_no_pool_is_a_valid_composition() {
    let value: Value = json!({ "meta": {}, "fonts": null });
    assert!(audio_lines_from_extract(&value).unwrap().is_empty());
}
