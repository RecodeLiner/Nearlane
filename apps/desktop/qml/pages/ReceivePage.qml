import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components" as UI

ScrollView {
    id: page
    required property QtObject tokens
    required property QtObject backend
    signal openSettings
    signal openAccount
    contentWidth: availableWidth
    clip: true
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: Math.min(page.availableWidth - 48, 800)
        x: (page.availableWidth - width) / 2
        spacing: 24
        Item {
            Layout.preferredHeight: 0
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: availability.implicitHeight + 48
            radius: page.tokens.largeRadius
            color: page.backend.receiving_active ? page.tokens.primaryContainer : page.tokens.surfaceContainer
            ColumnLayout {
                id: availability
                anchors.fill: parent
                anchors.margins: 24
                spacing: 16
                UI.BodyText {
                    tokens: page.tokens
                    role: "title"
                    color: page.backend.receiving_active ? page.tokens.primaryContainerText : page.tokens.surfaceText
                    text: page.backend.receiving_active ? qsTr("Доступен вашим устройствам") : !page.backend.account_authorized ? qsTr("Для приёма нужен вход") : page.backend.background_receiving ? qsTr("Подготовка приёма") : qsTr("Приём выключен")
                    Layout.fillWidth: true
                }
                UI.BodyText {
                    tokens: page.tokens
                    color: page.backend.receiving_active ? page.tokens.primaryContainerText : page.tokens.surfaceVariantText
                    text: page.backend.receiving_active ? qsTr("На телефоне выберите файл → Quick Share → Nearlane. Окно можно свернуть: приложение продолжит работать в трее.") : !page.backend.account_authorized ? qsTr("Подключите тот же Google-аккаунт, что и на телефоне.") : page.backend.background_receiving ? qsTr("Приём начнётся после подготовки приватных ключей. Если ожидание затянулось, проверьте состояние в настройках.") : qsTr("Включите приём, чтобы телефон мог найти этот компьютер.")
                    Layout.fillWidth: true
                }
                UI.ActionButton {
                    tokens: page.tokens
                    text: page.backend.receiving_active ? qsTr("Остановить приём") : !page.backend.account_authorized ? qsTr("Перейти к аккаунту") : qsTr("Включить приём")
                    primary: !page.backend.receiving_active
                    enabled: !page.backend.account_authorized || (!page.backend.transfer_busy && !page.backend.account_busy)
                    onClicked: {
                        if (!page.backend.account_authorized)
                            page.openAccount();
                        else if (page.backend.receiving_active)
                            page.backend.stop_receiving();
                        else
                            page.backend.start_receiving();
                    }
                }
            }
        }
        ColumnLayout {
            visible: page.backend.receive_pending
            Layout.fillWidth: true
            spacing: 12
            UI.BodyText {
                tokens: page.tokens
                text: qsTr("Принять файл?")
                role: "title"
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.incoming_offer
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                UI.ActionButton {
                    tokens: page.tokens
                    text: qsTr("Принять")
                    primary: true
                    onClicked: page.backend.decide_incoming(true)
                }
                UI.ActionButton {
                    tokens: page.tokens
                    text: qsTr("Отклонить")
                    onClicked: page.backend.decide_incoming(false)
                }
            }
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTr("Папка для файлов")
            role: "title"
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.receive_directory
            secondary: true
            Layout.fillWidth: true
        }
        UI.ActionButton {
            tokens: page.tokens
            text: qsTr("Настроить приём")
            symbolName: "settings"
            onClicked: page.openSettings()
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTr("Подтверждение передачи")
            role: "title"
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            text: page.backend.confirm_each_file ? qsTr("Для каждого файла появится запрос. Можно принять или отклонить передачу прямо в уведомлении.") : qsTr("Файлы от проверенных устройств вашего аккаунта принимаются автоматически. После сохранения появится уведомление.")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTr("Один файл до 64 МиБ. Существующие файлы не перезаписываются.")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.receive_status
            visible: page.backend.account_authorized
            Layout.fillWidth: true
        }
        Item {
            Layout.preferredHeight: 8
        }
    }
}
