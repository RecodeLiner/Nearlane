import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Button {
    id: control
    required property QtObject tokens
    property bool selected: false
    property string symbolName: "send"
    flat: true
    display: AbstractButton.TextUnderIcon
    icon.width: 24
    icon.height: 24
    icon.color: selected ? tokens.primaryContainerText : tokens.surfaceVariantText
    font.pointSize: tokens.label
    font.weight: selected ? Font.DemiBold : Font.Medium
    implicitHeight: Math.max(72, contentItem.implicitHeight + 16)
    implicitWidth: 96
    spacing: 6
    leftPadding: 8
    rightPadding: 8
    palette.buttonText: selected ? tokens.primaryContainerText : tokens.surfaceVariantText
    Accessible.name: text
    Accessible.role: Accessible.PageTab
    Accessible.checkable: true
    Accessible.checked: selected
    contentItem: ColumnLayout {
        spacing: 4
        Symbol {
            symbol: control.symbolName
            tint: control.selected ? control.tokens.primaryContainerText : control.tokens.surfaceVariantText
            Layout.alignment: Qt.AlignHCenter
        }
        BodyText {
            tokens: control.tokens
            text: control.text
            role: "label"
            color: control.selected ? control.tokens.primaryContainerText : control.tokens.surfaceVariantText
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
            Accessible.ignored: true
        }
    }
    background: Rectangle {
        radius: control.tokens.smallRadius + 4
        color: control.down ? control.tokens.surfaceHigh : control.selected ? control.tokens.primaryContainer : control.hovered ? control.tokens.surfaceHigh : "transparent"
        border.width: control.visualFocus ? 2 : 0
        border.color: control.tokens.primary
    }
}
