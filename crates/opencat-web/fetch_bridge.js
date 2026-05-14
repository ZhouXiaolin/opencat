export async function fetch_bytes_js(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${url}`);
    }
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
}
