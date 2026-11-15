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
                        enabled: backend.account_configured && !backend.account_busy && !backend.account_authorized && !backend.login_remembered
                        onClicked: backend.sign_in()
                    }
                    Button {
                        text: qsTr("Cancel")
                        visible: backend.login_cancellable
                        onClicked: backend.cancel_sign_in()
                    }
                    Button {
                        text: qsTr("Sign out")
                        visible: backend.account_authorized || backend.login_remembered
                        onClicked: backend.sign_out()
                    }
                    BusyIndicator {
                        visible: backend.account_busy
                        running: visible
                        implicitWidth: 28
                        implicitHeight: 28
                    }
                }
                Label { text: backend.saved_login_status; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                Button {
                    text: qsTr("Retry saved sign-in")
                    visible: backend.login_remembered && !backend.account_authorized
                    enabled: !backend.account_busy
                    onClicked: backend.restore_saved_sign_in()
                }
                Button {
                    text: qsTr("Select local sign-in configuration…")
                    visible: !backend.account_configured
                    onClicked: configDialog.open()
                }
                Label {
                    text: qsTr("Sign-in opens your browser once, then remembers offline access in the system credential store. Access tokens and device keys stay in memory. Use a test account for this experimental build.")
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
                    enabled: backend.account_authorized && !backend.account_busy && !backend.transfer_busy
                    onClicked: backend.prepare_own_devices()
                }
            }
        }
        Frame {
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 12
                Label { text: qsTr("Nearby devices"); font.bold: true }
                Label { text: backend.discovery_status; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                RowLayout {
                    Button {
                        text: qsTr("Find my devices")
                        enabled: backend.discovery_available && !backend.account_busy && !backend.discovery_busy && !backend.transfer_busy
                        onClicked: backend.find_own_devices()
                    }
                    Button {
                        text: qsTr("Stop search")
                        visible: backend.discovery_busy
                        onClicked: backend.cancel_discovery()
                    }
                    BusyIndicator { visible: backend.discovery_busy; running: visible; implicitWidth: 28; implicitHeight: 28 }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Button { text: qsTr("Choose file…"); enabled: backend.account_authorized && !backend.transfer_busy; onClicked: sendDialog.open() }
                    Label { text: backend.selected_send_name; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                }
                Repeater {
                    model: backend.own_devices
                    RowLayout {
                        required property string modelData
                        required property int index
                        Layout.fillWidth: true
                        Label { text: parent.modelData; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                        Button {
                            text: qsTr("Send file")
                            enabled: backend.account_authorized && !backend.transfer_busy && !backend.account_busy && backend.selected_send_name.length > 0
                            onClicked: backend.send_selected_file(parent.index)
                        }
                    }
                }
                Label { text: backend.transfer_status; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                Button { text: qsTr("Cancel transfer"); visible: backend.transfer_busy; onClicked: backend.cancel_transfer() }
                Label {
                    text: qsTr("Experimental: one file up to 64 MiB. Sending starts only after both devices verify their private keys and the phone accepts the offer.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pixelSize: 12
                }
                Label {
                    text: qsTr("Use the same local network and Google account on both devices. Keep the phone unlocked with Quick Share open and its existing private visibility. This build searches Wi-Fi LAN only; it does not use Everyone visibility.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    font.pixelSize: 12
                }
            }
        }
        Label { text: backend.status; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: palette.mid }
        Frame {
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 12
                CheckBox { id: showCertificates; text: qsTr("Show received certificates") }
                Label {
                    visible: showCertificates.checked
                    text: qsTr("This computer normally has three certificates for consecutive validity periods. Removing that set would stop private sharing. Google withholds the name-decryption key from downloads, so other names appear after nearby discovery. Older registrations can remain until expiry; Sign out requests cleanup of the current registration.")
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
                Repeater {
                    model: showCertificates.checked ? backend.certificate_rows : []
                    Label {
                        required property string modelData
                        text: modelData
                        textFormat: Text.PlainText
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
        Label {
            text: qsTr("Receiving files and Contacts mode are still being implemented. This computer stays hidden; outgoing transfers use the selected own device.")
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
    FileDialog {
        id: sendDialog
        title: qsTr("Choose a file to send")
        fileMode: FileDialog.OpenFile
        onAccepted: backend.select_send_file(selectedFile)
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
