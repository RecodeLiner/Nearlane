import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "../components" as UI

ScrollView {
    id: page
    required property QtObject tokens
    required property QtObject backend
    property int selectedDevice: -1
    property bool attemptedTransfer: false
    signal chooseFile
    signal openSettings
    function focusFilePicker() {
        filePicker.forceActiveFocus(Qt.TabFocusReason);
        return filePicker.visualFocus && filePicker.nextItemInFocusChain(true) !== filePicker;
    }
    contentWidth: availableWidth
    clip: true
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    Connections {
        target: page.backend
        function onOwn_devicesChanged() {
            page.selectedDevice = -1;
        }
    }
    ColumnLayout {
        width: Math.min(page.availableWidth - 48, 800)
        x: (page.availableWidth - width) / 2
        spacing: 24
        Item {
            Layout.preferredHeight: 0
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTranslate("Ui", "Choose a file and the device to send it to.")
            secondary: true
            Layout.fillWidth: true
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: attachment.implicitHeight + 40
            radius: page.tokens.largeRadius
            color: page.tokens.surfaceContainer
            ColumnLayout {
                id: attachment
                anchors.fill: parent
                anchors.margins: 20
                spacing: 16
                UI.BodyText {
                    tokens: page.tokens
                    role: "title"
                    text: page.backend.selected_send_name.length > 0 ? page.backend.selected_send_name : qsTranslate("Ui", "What would you like to send?")
                    Layout.fillWidth: true
                }
                UI.BodyText {
                    tokens: page.tokens
                    secondary: true
                    text: qsTranslate("Ui", "One file up to 64 GiB. The transfer will be encrypted.")
                    Layout.fillWidth: true
                }
                UI.ActionButton {
                    id: filePicker
                    tokens: page.tokens
                    text: page.backend.selected_send_name.length > 0 ? qsTranslate("Ui", "Choose another file") : qsTranslate("Ui", "Choose file")
                    primary: page.backend.selected_send_name.length === 0
                    symbolName: "file"
                    enabled: page.backend.account_authorized && !page.backend.transfer_busy
                    onClicked: page.chooseFile()
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            UI.BodyText {
                tokens: page.tokens
                text: qsTranslate("Ui", "Nearby devices")
                role: "title"
                Layout.fillWidth: true
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Refresh")
                enabled: page.backend.discovery_available && !page.backend.account_busy && !page.backend.discovery_busy && !page.backend.transfer_busy
                onClicked: page.backend.find_own_devices()
            }
        }
        ProgressBar {
            Layout.fillWidth: true
            visible: page.backend.discovery_busy || page.backend.private_busy
            indeterminate: true
            Accessible.name: qsTranslate("Ui", "Searching for devices")
        }
        ColumnLayout {
            visible: page.backend.own_devices.length === 0
            Layout.fillWidth: true
            spacing: 12
            UI.BodyText {
                tokens: page.tokens
                text: !page.backend.account_authorized ? qsTranslate("Ui", "Connect your Google account") : page.backend.private_busy ? qsTranslate("Ui", "Preparing your devices…") : page.backend.discovery_busy ? qsTranslate("Ui", "Looking for your devices…") : qsTranslate("Ui", "No devices nearby yet")
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                secondary: true
                text: !page.backend.account_authorized ? qsTranslate("Ui", "Sign in to the same account you use on your phone.") : qsTranslate("Ui", "Connect both devices to the same network. In Quick Share on your phone, set visibility to “Your devices”.")
                Layout.fillWidth: true
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Go to account")
                visible: !page.backend.account_authorized
                onClicked: page.openSettings()
            }
        }
        Repeater {
            model: page.backend.bluetooth_devices
            UI.BodyText {
                required property string modelData
                tokens: page.tokens
                text: qsTranslate("Ui", "%1 · found via Bluetooth, waiting for Wi-Fi").arg(modelData)
                secondary: true
                Layout.fillWidth: true
            }
        }
        ColumnLayout {
            spacing: 4
            Layout.fillWidth: true
            Repeater {
                model: page.backend.own_devices
                RadioDelegate {
                    required property string modelData
                    required property int index
                    Layout.fillWidth: true
                    implicitHeight: Math.max(64, contentItem.implicitHeight + 24)
                    text: modelData
                    font.pointSize: page.tokens.body
                    checked: page.selectedDevice === index
                    enabled: !page.backend.transfer_busy
                    Accessible.name: modelData
                    Accessible.description: qsTranslate("Ui", "Select a device to send to")
                    onClicked: page.selectedDevice = index
                    Material.roundedScale: Material.MediumScale
                    contentItem: UI.BodyText {
                        tokens: page.tokens
                        text: parent.text
                        verticalAlignment: Text.AlignVCenter
                        rightPadding: parent.indicator.width + 16
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            visible: page.backend.own_devices.length > 0
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Send a file")
                primary: true
                symbolName: "send"
                enabled: page.selectedDevice >= 0 && page.backend.selected_send_name.length > 0 && page.backend.account_authorized && !page.backend.account_busy && !page.backend.transfer_busy && !page.backend.receive_pending
                onClicked: {
                    page.attemptedTransfer = true;
                    page.backend.send_selected_file(page.selectedDevice);
                }
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Cancel")
                visible: page.backend.transfer_busy
                onClicked: page.backend.cancel_transfer()
            }
        }
        ColumnLayout {
            visible: page.attemptedTransfer || page.backend.transfer_busy
            Layout.fillWidth: true
            spacing: 12
            ProgressBar {
                Layout.fillWidth: true
                indeterminate: true
                visible: page.backend.transfer_busy
                Accessible.name: qsTranslate("Ui", "Transferring file")
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.transfer_status
                Layout.fillWidth: true
                Accessible.name: text
            }
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "Only devices on your account appear here. Private keys are verified when connecting. Files are transferred over Wi-Fi.")
            Layout.fillWidth: true
        }
        UI.BodyText { tokens: page.tokens; text: page.backend.bluetooth_status; secondary: true; role: "label"; Layout.fillWidth: true }
        Item {
            Layout.preferredHeight: 8
        }
    }
}
