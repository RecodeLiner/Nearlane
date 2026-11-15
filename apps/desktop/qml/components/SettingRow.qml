import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: row
    required property QtObject tokens
    required property string title
    property string description: ""
    property bool checked: false
    signal toggled(bool value)
    implicitHeight: Math.max(64, textColumn.implicitHeight + 24, toggle.implicitHeight)
    RowLayout {
        anchors.fill: parent
        spacing: 16
        ColumnLayout {
            id: textColumn
            Layout.fillWidth: true
            spacing: 4
            BodyText {
                tokens: row.tokens
                text: row.title
                Layout.fillWidth: true
            }
            BodyText {
                tokens: row.tokens
                text: row.description
                secondary: true
                role: "label"
                visible: text.length > 0
                Layout.fillWidth: true
            }
        }
        Switch {
            id: toggle
            checked: row.checked
            Accessible.name: row.title
            Accessible.description: row.description
            onClicked: row.toggled(checked)
        }
    }
}
