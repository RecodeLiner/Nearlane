import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Qt.labs.platform as Platform
import org.nearlane.app

ApplicationWindow {
    id: window
    width: 760
    height: 600
    minimumWidth: 560
    minimumHeight: 520
    visible: true
    title: "Nearlane"
    property bool quitting: false
    property bool incomingPending: backend.receive_pending
    property bool actionableNotification: backend.actionable_notification
    property int savedFiles: backend.received_count
    function showWindow() { window.showNormal(); window.raise(); window.requestActivate() }
    function quitApp() { quitting = true; backend.shutdown(); Qt.quit() }
    onClosing: function(event) {
        if (!quitting && (!backend.smoke_test || backend.tray_test) && tray.available) {
            event.accepted = false
            window.hide()
        } else {
            backend.shutdown()
            Qt.quit()
        }
    }
    function fallbackIncomingNotification() {
        if (incomingPending && !actionableNotification && !backend.smoke_test) {
            if (tray.available && tray.supportsMessages)
                tray.showMessage(qsTr("Incoming file"), qsTr("A verified own device wants to send a file. Open Nearlane to accept or decline."), Platform.SystemTrayIcon.Information, 10000)
            else showWindow()
        }
    }
    onIncomingPendingChanged: fallbackIncomingNotification()
    onActionableNotificationChanged: { if (!actionableNotification) fallbackIncomingNotification() }
    onSavedFilesChanged: {
        if (savedFiles > 0 && !backend.smoke_test && tray.available && tray.supportsMessages)
            tray.showMessage(qsTr("File received"), qsTr("The file was saved in your selected folder."), Platform.SystemTrayIcon.Information, 6000)
    }

    Platform.SystemTrayIcon {
        id: tray
        visible: !backend.smoke_test || backend.tray_test
        icon.source: "qrc:/nearlane/icon.svg"
        tooltip: backend.receiving_active ? qsTr("Nearlane · Receiving from your devices") : qsTr("Nearlane · Running in background")
        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger || reason === Platform.SystemTrayIcon.DoubleClick)
                window.showWindow()
        }
        onMessageClicked: window.showWindow()
        menu: Platform.Menu {
            Platform.MenuItem { text: qsTr("Open Nearlane"); onTriggered: window.showWindow() }
            Platform.MenuItem {
                text: qsTr("Receive from own devices")
                checkable: true
                checked: backend.background_receiving
                onTriggered: backend.configure_receiving(!backend.background_receiving, backend.confirm_each_file)
            }
            Platform.MenuItem {
                text: qsTr("Find devices continuously")
                checkable: true
                checked: backend.continuous_discovery
                onTriggered: backend.set_background_discovery(!backend.continuous_discovery)
            }
            Platform.MenuSeparator {}
            Platform.MenuItem { text: qsTr("Quit"); onTriggered: window.quitApp() }
        }
    }

    Backend { id: backend }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            Label { text: "Nearlane"; font.pixelSize: 22; font.bold: true; Layout.fillWidth: true }
            Label { text: backend.receiving_active ? qsTr("Own devices · Receiving") : qsTr("Hidden"); color: palette.mid }
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
                    text: qsTr("After sign-in, Nearlane registers this computer and exchanges temporary certificates for your own devices. When receiving is enabled, only verified devices on your account can send files to this computer.")
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
                CheckBox {
                    text: qsTr("Find my devices continuously")
                    checked: backend.continuous_discovery
                    onClicked: backend.set_background_discovery(checked)
                }
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
                            enabled: backend.account_authorized && !backend.transfer_busy && !backend.receive_pending && !backend.account_busy && backend.selected_send_name.length > 0
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
                    text: qsTr("Use the same local network and Google account on both devices, with private Quick Share visibility. This build uses Wi-Fi LAN only; Bluetooth/BLE background discovery is not ready. The phone's Receive screen is not required and can temporarily make it visible to everyone.")
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
        Frame {
            Layout.fillWidth: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 12
                Label { text: qsTr("Receive on this computer"); font.bold: true }
                Switch {
                    text: qsTr("Always receive from my devices")
                    checked: backend.background_receiving
                    onClicked: backend.configure_receiving(checked, backend.confirm_each_file)
                }
                CheckBox {
                    text: qsTr("Ask before receiving each file")
                    checked: backend.confirm_each_file
                    onClicked: backend.configure_receiving(backend.background_receiving, checked)
                }
                Label {
                    text: qsTr("Settings are saved. By default, Nearlane receives automatically from verified devices on your Google account whenever the app is running, including in the tray. Sign-out and Quit stop receiving.")
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12
                }
                Label { visible: text.length > 0; text: backend.preferences_status; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                RowLayout {
                    Layout.fillWidth: true
                    Button { text: qsTr("Choose folder…"); enabled: !backend.receiving_active; onClicked: receiveFolder.open() }
                    Label { text: backend.receive_directory; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                }
                RowLayout {
                    Button {
                        text: qsTr("Start receiving")
                        enabled: backend.discovery_available && !backend.account_busy && !backend.receiving_active && !backend.transfer_busy && backend.receive_directory.length > 0
                        onClicked: backend.start_receiving()
                    }
                    Button { text: qsTr("Stop receiving"); visible: backend.receiving_active; onClicked: backend.stop_receiving() }
                }
                Label { text: backend.receive_status; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                Label { visible: backend.receive_pending; text: backend.incoming_offer; textFormat: Text.PlainText; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                RowLayout {
                    visible: backend.receive_pending
                    Button { text: qsTr("Accept file"); onClicked: backend.decide_incoming(true) }
                    Button { text: qsTr("Decline"); onClicked: backend.decide_incoming(false) }
                }
                Label {
                    text: qsTr("Only verified devices from your account may send a file. Enable confirmation above to accept or decline each offer, including from a Linux notification. One file up to 64 MiB; existing files are never replaced. Stop receiving hides this computer and saves that choice for the next launch.")
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12
                }
            }
        }
        Label {
            text: qsTr("Contacts mode is still being implemented. This computer is discoverable by your own devices only while receiving is enabled.")
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
    FolderDialog {
        id: receiveFolder
        title: qsTr("Choose where to save received files")
        onAccepted: backend.select_receive_directory(selectedFolder)
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
            if (backend.tray_test) {
                if (!tray.available) throw new Error("System tray is not available")
                if (!backend.receive_directory.endsWith("/Nearlane") && !backend.receive_directory.endsWith("\\Nearlane"))
                    throw new Error("Default receiving folder is missing")
                window.close()
                if (window.visible) throw new Error("Closing did not hide to tray")
                window.showWindow()
                if (!window.visible) throw new Error("Window did not restore from tray")
                if (tray.supportsMessages)
                    tray.showMessage("Nearlane", "Background notification check", Platform.SystemTrayIcon.Information, 2500)
                console.log("smoke: tray close/show and notification checked")
            }
            Qt.quit()
        }
    }
}
