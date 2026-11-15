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
            text: qsTr("Выберите файл и устройство, которому хотите его отправить.")
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
                    text: page.backend.selected_send_name.length > 0 ? page.backend.selected_send_name : qsTr("Что отправим?")
                    Layout.fillWidth: true
                }
                UI.BodyText {
                    tokens: page.tokens
                    secondary: true
                    text: qsTr("Один файл до 64 МиБ. Передача будет зашифрована.")
                    Layout.fillWidth: true
                }
                UI.ActionButton {
                    id: filePicker
                    tokens: page.tokens
                    text: page.backend.selected_send_name.length > 0 ? qsTr("Выбрать другой файл") : qsTr("Выбрать файл")
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
                text: qsTr("Устройства рядом")
                role: "title"
                Layout.fillWidth: true
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Обновить")
                enabled: page.backend.discovery_available && !page.backend.account_busy && !page.backend.discovery_busy && !page.backend.transfer_busy
                onClicked: page.backend.find_own_devices()
            }
        }
        ProgressBar {
            Layout.fillWidth: true
            visible: page.backend.discovery_busy || page.backend.private_busy
            indeterminate: true
            Accessible.name: qsTr("Поиск устройств")
        }
        ColumnLayout {
            visible: page.backend.own_devices.length === 0
            Layout.fillWidth: true
            spacing: 12
            UI.BodyText {
                tokens: page.tokens
                text: !page.backend.account_authorized ? qsTr("Подключите Google-аккаунт") : page.backend.private_busy ? qsTr("Подготавливаем ваши устройства…") : page.backend.discovery_busy ? qsTr("Ищем ваши устройства…") : qsTr("Пока нет устройств рядом")
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                secondary: true
                text: !page.backend.account_authorized ? qsTr("Войдите в тот же аккаунт, который используется на телефоне.") : qsTr("Подключите устройства к одной сети. В Quick Share на телефоне выберите видимость «Ваши устройства».")
                Layout.fillWidth: true
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Перейти к аккаунту")
                visible: !page.backend.account_authorized
                onClicked: page.openSettings()
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
                    Accessible.description: qsTr("Выбрать устройство для отправки")
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
                text: qsTr("Отправить файл")
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
                text: qsTr("Отменить")
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
                Accessible.name: qsTr("Передача файла")
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
            text: qsTr("В списке - только устройства вашего аккаунта. Приватные ключи проверяются при подключении. Bluetooth пока не поддерживается.")
            Layout.fillWidth: true
        }
        Item {
            Layout.preferredHeight: 8
        }
    }
}
