import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import org.nearlane.app

ApplicationWindow {
    id: window
    width: 760
    height: 600
    minimumWidth: 560
    minimumHeight: 520
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

    ScrollView {
        id: page
        anchors.fill: parent
        anchors.margins: 28
        contentWidth: availableWidth
        clip: true

    ColumnLayout {
        width: page.availableWidth
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
                Label {
                    text: backend.account_authorized ? qsTr("Quick Share permission granted") : qsTr("Connect your Google account")
                    font.bold: true
                }
                Label {
                    text: backend.account_status
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    Button {
                        text: qsTr("Sign in with Google")
                        enabled: backend.account_configured && !backend.account_busy && !backend.account_authorized
                        onClicked: backend.sign_in()
                    }
                    Button {
                        text: qsTr("Cancel")
                        visible: backend.account_busy && !backend.private_busy
                        onClicked: backend.cancel_sign_in()
                    }
                    Button {
                        text: qsTr("Sign out")
                        visible: backend.account_authorized
                        onClicked: backend.sign_out()
                    }
                    BusyIndicator {
                        visible: backend.account_busy
                        running: visible
                        implicitWidth: 28
                        implicitHeight: 28
                    }
                }
                Button {
                    text: qsTr("Select local sign-in configuration…")
                    visible: !backend.account_configured
                    onClicked: configDialog.open()
                }
                Label {
                    text: qsTr("Sign-in opens your browser, then prepares temporary own-device certificates. Use a test account for this experimental build. Tokens and private keys stay in memory.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pixelSize: 12
                }
                Label {
                    text: qsTr("This compatibility experiment uses the locally imported Google desktop configuration. Google's consent screen may name Quick Share. Nearlane is an independent app.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pixelSize: 12
                }
            }
        }
        Frame {
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 12
                Label { text: qsTr("Your own devices"); font.bold: true }
                Label { text: backend.private_status; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                Label {
                    text: qsTr("After sign-in, Nearlane registers this computer with Google and exchanges temporary certificates for your own devices. It stays hidden while nearby connection support is being completed.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pixelSize: 12
                }
                Button {
                    text: qsTr("Retry device setup")
                    enabled: backend.account_authorized && !backend.account_busy
                    onClicked: backend.prepare_own_devices()
                }
            }
        }
        Label { text: backend.status; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: palette.mid }
        Label {
            text: qsTr("Signing in does not enable file sharing yet. Contact verification and transfers are still being implemented.")
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
        }
        Label { text: qsTr("Independent project. Not affiliated with Google or Samsung."); font.pixelSize: 11; wrapMode: Text.WordWrap; Layout.fillWidth: true }
    }
    }

    FileDialog {
        id: configDialog
        title: qsTr("Select local sign-in configuration")
        nameFilters: [qsTr("JSON files (*.json)")]
        onAccepted: backend.load_config(selectedFile)
    }

    Timer {
        interval: 100
        running: true
        repeat: true
        onTriggered: backend.poll_account()
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
