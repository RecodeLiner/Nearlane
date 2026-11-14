import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.nearlane.app

ApplicationWindow {
    id: window
    width: 760
    height: 520
    minimumWidth: 560
    minimumHeight: 420
    visible: true
    title: "Nearlane"

    Backend { id: backend }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            Label { text: "Nearlane"; font.pixelSize: 22; font.bold: true; Layout.fillWidth: true }
            Label { text: qsTr("Hidden"); color: palette.mid }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        spacing: 18
        Label { text: qsTr("Your devices, nearby"); font.pixelSize: 28; font.bold: true }
        Label {
            text: qsTr("A private way to share with Quick Share devices.")
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Frame {
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 12
                Label { text: qsTr("Account connection is not ready"); font.bold: true }
                Label {
                    text: qsTr("This development build stays hidden. Contact verification and file transfers are not enabled yet.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
                Button { text: qsTr("Check contacts support"); onClicked: backend.check_contacts() }
            }
        }
        Label { text: backend.status; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: palette.mid }
        Item { Layout.fillHeight: true }
        Label { text: qsTr("Independent project. Not affiliated with Google or Samsung."); font.pixelSize: 11; wrapMode: Text.WordWrap; Layout.fillWidth: true }
    }

    Timer {
        interval: 800
        running: backend.smoke_test
        onTriggered: {
            backend.check_contacts()
            Qt.quit()
        }
    }
}
