import QtQuick
import QtQuick.Controls

Label {
    required property QtObject tokens
    property string role: "body"
    property bool secondary: false
    textFormat: Text.PlainText
    wrapMode: Text.Wrap
    color: secondary ? tokens.surfaceVariantText : tokens.surfaceText
    font.pointSize: role === "headline" ? tokens.headline : role === "title" ? tokens.title : role === "label" ? tokens.label : tokens.body
    font.weight: role === "title" || role === "label" ? Font.Medium : Font.Normal
    Accessible.ignored: text.length === 0
}
