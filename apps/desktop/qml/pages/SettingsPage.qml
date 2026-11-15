import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components" as UI

ScrollView {
    id: page
    required property QtObject tokens
    required property QtObject backend
    signal chooseFolder
    signal chooseConfig
    function showAccount() {
        contentItem.contentY = Math.max(0, Math.min(accountHeading.y, contentItem.contentHeight - availableHeight));
    }
    function showGeneral() {
        contentItem.contentY = 0;
    }
    contentWidth: availableWidth
    clip: true
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: Math.min(page.availableWidth - 48, 800)
        x: (page.availableWidth - width) / 2
        spacing: 16
        Item {
            Layout.preferredHeight: 0
        }
        UI.BodyText {
            tokens: page.tokens
            role: "title"
            text: qsTr("Приём и обнаружение")
            Layout.fillWidth: true
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTr("Принимать от своих устройств")
            description: qsTr("Включать приём после входа и продолжать работу в трее.")
            checked: page.backend.background_receiving
            onToggled: value => page.backend.configure_receiving(value, page.backend.confirm_each_file)
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTr("Спрашивать перед получением")
            description: qsTr("Кнопки «Принять» и «Отклонить» в окне и уведомлении.")
            checked: page.backend.confirm_each_file
            onToggled: value => page.backend.configure_receiving(page.backend.background_receiving, value)
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTr("Искать устройства в фоне")
            description: qsTr("Обновлять список устройств вашего аккаунта в локальной сети.")
            checked: page.backend.continuous_discovery
            onToggled: value => page.backend.set_background_discovery(value)
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.preferences_status
            color: page.tokens.error
            visible: text.length > 0
            Layout.fillWidth: true
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: page.tokens.outlineVariant
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTr("Сохранять файлы в")
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
            text: qsTr("Изменить папку")
            symbolName: "folder"
            enabled: !page.backend.receiving_active
            onClicked: page.chooseFolder()
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: page.backend.receiving_active ? qsTr("Чтобы изменить папку, сначала выключите приём выше.") : qsTr("Папка и параметры приёма сохраняются между запусками.")
            Layout.fillWidth: true
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: page.tokens.outlineVariant
        }
        UI.BodyText {
            id: accountHeading
            tokens: page.tokens
            text: qsTr("Google-аккаунт")
            role: "title"
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.account_authorized ? qsTr("Аккаунт подключён") : page.backend.account_busy ? qsTr("Выполняется вход…") : qsTr("Аккаунт не подключён")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            text: qsTr("На телефоне и компьютере нужен один аккаунт. Nearlane запоминает вход в системном хранилище учётных данных.")
            Layout.fillWidth: true
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Войти через Google")
                primary: true
                visible: !page.backend.account_authorized && !page.backend.login_remembered
                enabled: page.backend.account_configured && !page.backend.account_busy
                onClicked: page.backend.sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Повторить вход")
                visible: page.backend.login_remembered && !page.backend.account_authorized
                enabled: !page.backend.account_busy
                onClicked: page.backend.restore_saved_sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Отменить вход")
                visible: page.backend.login_cancellable
                onClicked: page.backend.cancel_sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Выбрать конфигурацию входа")
                visible: !page.backend.account_configured
                onClicked: page.chooseConfig()
            }
        }
        ProgressBar {
            Layout.fillWidth: true
            visible: page.backend.account_busy
            indeterminate: true
            Accessible.name: qsTr("Вход в аккаунт")
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.account_status
            visible: !page.backend.account_authorized
            Layout.fillWidth: true
        }
        UI.ActionButton {
            tokens: page.tokens
            text: qsTr("Выйти из аккаунта")
            visible: page.backend.account_authorized || page.backend.login_remembered
            onClicked: page.backend.sign_out()
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: page.tokens.outlineVariant
        }
        CheckBox {
            id: details
            text: qsTr("Показать диагностику")
            font.pointSize: page.tokens.body
            Accessible.description: qsTr("Состояние входа, приватных ключей и сертификаты устройств")
        }
        ColumnLayout {
            visible: details.checked
            Layout.fillWidth: true
            spacing: 16
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.private_status
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.discovery_status
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.saved_login_status
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.receive_status
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.transfer_status
                Layout.fillWidth: true
            }
            UI.BodyText {
                tokens: page.tokens
                text: page.backend.status
                Layout.fillWidth: true
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Повторить настройку устройств")
                enabled: page.backend.account_authorized && !page.backend.account_busy && !page.backend.transfer_busy && !page.backend.receiving_active
                onClicked: page.backend.prepare_own_devices()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Остановить поиск")
                visible: page.backend.discovery_busy
                onClicked: page.backend.cancel_discovery()
            }
            UI.BodyText {
                tokens: page.tokens
                secondary: true
                role: "label"
                text: qsTr("У этого компьютера обычно три сертификата для последовательных периодов. Названия других устройств появляются после обнаружения рядом. Старые регистрации могут оставаться до истечения срока.")
                Layout.fillWidth: true
            }
            Repeater {
                model: details.checked ? page.backend.certificate_rows : []
                UI.BodyText {
                    required property string modelData
                    tokens: page.tokens
                    text: modelData
                    Layout.fillWidth: true
                }
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTr("Проверить поддержку контактов")
                onClicked: page.backend.check_contacts()
            }
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTr("Экспериментальная версия: Wi-Fi в одной сети, один файл до 64 МиБ. Bluetooth и режим контактов пока недоступны. Для проверки используйте тестовый аккаунт.")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTr("Используется локально импортированная конфигурация Quick Share; это имя может отображаться на странице согласия. Nearlane - независимый проект, не связанный с Google или Samsung.")
            Layout.fillWidth: true
        }
        Item {
            Layout.preferredHeight: 8
        }
    }
}
