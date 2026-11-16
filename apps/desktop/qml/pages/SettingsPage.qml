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
            text: qsTranslate("Ui", "Language")
            Layout.fillWidth: true
        }
        ComboBox {
            id: languagePicker
            Layout.fillWidth: true
            font.pointSize: page.tokens.body
            textRole: "label"
            valueRole: "code"
            model: [
                { code: "system", label: qsTranslate("Ui", "System default") },
                { code: "ru", label: "Русский" },
                { code: "en", label: "English" }
            ]
            currentIndex: Math.max(0, ["system", "ru", "en"].indexOf(page.backend.language))
            Accessible.name: qsTranslate("Ui", "Interface language")
            onActivated: page.backend.select_language(currentValue)
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "The language changes immediately and is remembered for next time.")
            Layout.fillWidth: true
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: page.tokens.outlineVariant
        }
        UI.BodyText {
            tokens: page.tokens
            role: "title"
            text: qsTranslate("Ui", "Receiving and discovery")
            Layout.fillWidth: true
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTranslate("Ui", "Receive from your devices")
            description: qsTranslate("Ui", "Enable receiving after sign-in and keep running in the system tray.")
            checked: page.backend.background_receiving
            onToggled: value => page.backend.configure_receiving(value, page.backend.confirm_each_file)
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTranslate("Ui", "Ask before receiving")
            description: qsTranslate("Ui", "Show Accept and Decline buttons in the window and notification.")
            checked: page.backend.confirm_each_file
            onToggled: value => page.backend.configure_receiving(page.backend.background_receiving, value)
        }
        UI.SettingRow {
            tokens: page.tokens
            Layout.fillWidth: true
            title: qsTranslate("Ui", "Find devices in the background")
            description: qsTranslate("Ui", "Discover devices over Wi-Fi and Bluetooth. Transfer files over Wi-Fi.")
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
            text: qsTranslate("Ui", "Save files to")
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
            text: qsTranslate("Ui", "Change folder")
            symbolName: "folder"
            enabled: !page.backend.receiving_active
            onClicked: page.chooseFolder()
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: page.backend.receiving_active ? qsTranslate("Ui", "To change the folder, first turn off receiving above.") : qsTranslate("Ui", "The folder and receiving preferences are remembered between sessions.")
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
            text: qsTranslate("Ui", "Google account")
            role: "title"
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.account_authorized ? qsTranslate("Ui", "Account connected") : page.backend.account_busy ? qsTranslate("Ui", "Signing in…") : qsTranslate("Ui", "Account not connected")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            text: qsTranslate("Ui", "Use the same account on your phone and computer. Nearlane remembers your sign-in in the system credential store.")
            Layout.fillWidth: true
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Sign in with Google")
                primary: true
                visible: !page.backend.account_authorized && !page.backend.login_remembered
                enabled: page.backend.account_configured && !page.backend.account_busy
                onClicked: page.backend.sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Retry sign-in")
                visible: page.backend.login_remembered && !page.backend.account_authorized
                enabled: !page.backend.account_busy
                onClicked: page.backend.restore_saved_sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Cancel sign-in")
                visible: page.backend.login_cancellable
                onClicked: page.backend.cancel_sign_in()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Choose sign-in configuration")
                visible: !page.backend.account_authorized
                enabled: !page.backend.account_busy
                onClicked: page.chooseConfig()
            }
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "Select a local OAuth JSON file. Its application parameters will be saved privately for future launches.")
            visible: !page.backend.account_authorized
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "Configuration file: %1").arg(page.backend.config_path)
            visible: page.backend.config_path.length > 0
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.config_status
            color: page.backend.config_error ? page.tokens.error : page.tokens.surfaceText
            visible: text.length > 0
            Layout.fillWidth: true
        }
        ProgressBar {
            Layout.fillWidth: true
            visible: page.backend.account_busy
            indeterminate: true
            Accessible.name: qsTranslate("Ui", "Signing in")
        }
        UI.BodyText {
            tokens: page.tokens
            text: page.backend.account_status
            visible: !page.backend.account_authorized
            Layout.fillWidth: true
        }
        UI.ActionButton {
            tokens: page.tokens
            text: qsTranslate("Ui", "Sign out")
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
            text: qsTranslate("Ui", "Show diagnostics")
            font.pointSize: page.tokens.body
            Accessible.description: qsTranslate("Ui", "Sign-in status, private keys and device certificates")
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
                text: qsTranslate("Ui", "Retry device setup")
                enabled: page.backend.account_authorized && !page.backend.account_busy && !page.backend.transfer_busy && !page.backend.receiving_active
                onClicked: page.backend.prepare_own_devices()
            }
            UI.ActionButton {
                tokens: page.tokens
                text: qsTranslate("Ui", "Stop searching")
                visible: page.backend.discovery_busy
                onClicked: page.backend.cancel_discovery()
            }
            UI.BodyText {
                tokens: page.tokens
                secondary: true
                role: "label"
                text: qsTranslate("Ui", "This computer usually has three certificates for consecutive periods. Other device names appear once discovered nearby. Old registrations may remain until their certificates expire.")
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
                text: qsTranslate("Ui", "Check contacts support")
                onClicked: page.backend.check_contacts()
            }
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "Experimental version: one file up to 64 GiB, over Wi-Fi on the same network. Bluetooth helps discovery; Contacts mode is not available yet. Use a test account for testing.")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "Uses a locally imported Quick Share configuration; that name may appear on the consent page. Nearlane is an independent project, not affiliated with Google or Samsung.")
            Layout.fillWidth: true
        }
        Item {
            Layout.preferredHeight: 8
        }
    }
}
