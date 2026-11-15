import QtQuick

Image {
    property string symbol: "send"
    property color tint: "#000000"
    readonly property var paths: ({
            send: '<path d="m3 4 18 8-18 8 3-8-3-8Zm3 8h15"/>',
            receive: '<path d="M12 3v12m-5-5 5 5 5-5M4 15v5h16v-5"/>',
            settings: '<path d="M4 6h3m6 0h7M4 12h9m6 0h1M4 18h3m6 0h7"/><circle cx="10" cy="6" r="3"/><circle cx="16" cy="12" r="3"/><circle cx="10" cy="18" r="3"/>',
            file: '<path d="M13 3H5v18h14V9l-6-6Zm0 0v6h6M8 13h8m-8 4h5"/>',
            folder: '<path d="M3 6h6l2 3h10v11H3V6Z"/>',
            lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2"/>'
        })
    source: "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="' + tint + '" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">' + (paths[symbol] || paths.file) + '</g></svg>')
    sourceSize.width: 24
    sourceSize.height: 24
    width: 24
    height: 24
    Accessible.ignored: true
}
