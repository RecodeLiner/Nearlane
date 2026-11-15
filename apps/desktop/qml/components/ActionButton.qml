import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

Button {
    id: control
    required property QtObject tokens
    property bool primary: false
    property string symbolName: ""
    Layout.maximumWidth: tokens.controlMaxWidth
    width: Math.min(implicitWidth, tokens.controlMaxWidth)
    highlighted: primary
    flat: !primary
    font.pointSize: tokens.label
    implicitHeight: Math.max(48, contentItem.implicitHeight + topPadding + bottomPadding)
    implicitWidth: Math.max(120, label.implicitWidth + leftPadding + rightPadding + (symbol.visible ? 32 : 0))
    leftPadding: 20
    rightPadding: 20
    Material.roundedScale: Material.FullScale
    Material.elevation: 0
    Material.accent: tokens.primary
    Material.foreground: primary ? tokens.primaryText : tokens.primary
    Accessible.name: text
    // These flat surfaces have no shadow. Disabling the effect layer also keeps
    // Qt's Material background visible under its software scene-graph backend.
    Component.onCompleted: {
        if (background)
            background.layer.enabled = false;
    }
    Rectangle {
        anchors.fill: parent
        anchors.margins: 3
        radius: height / 2
        color: "transparent"
        border.width: control.visualFocus ? 2 : 0
        border.color: control.primary ? control.tokens.primaryText : control.tokens.primary
        visible: control.visualFocus
    }
    contentItem: RowLayout {
        spacing: 8
        Symbol {
            id: symbol
            symbol: control.symbolName
            visible: control.symbolName.length > 0
            tint: label.color
        }
        BodyText {
            id: label
            tokens: control.tokens
            role: "label"
            text: control.text
            color: !control.enabled ? control.tokens.surfaceVariantText : control.primary ? control.tokens.primaryText : control.tokens.primary
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
            Accessible.ignored: true
        }
    }
}
