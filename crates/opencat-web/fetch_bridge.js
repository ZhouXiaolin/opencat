const PROXY_MAP = [
    { prefix: 'http://127.0.0.1:8080/', replace: '/assets-proxy/' },
];

export async function fetch_bytes_js(url) {
    for (const rule of PROXY_MAP) {
        if (url.startsWith(rule.prefix)) {
            url = rule.replace + url.slice(rule.prefix.length);
            break;
        }
    }
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${url}`);
    }
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
}
