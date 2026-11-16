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
                    text: page.backend.receiving_active ? qsTranslate("Ui", "Available to your devices") : !page.backend.account_authorized ? qsTranslate("Ui", "Sign in to receive files") : page.backend.background_receiving ? qsTranslate("Ui", "Preparing to receive") : qsTranslate("Ui", "Receiving is off")
                    Layout.fillWidth: true
                }
                UI.BodyText {
                    tokens: page.tokens
                    color: page.backend.receiving_active ? page.tokens.primaryContainerText : page.tokens.surfaceVariantText
                    text: page.backend.receiving_active ? qsTranslate("Ui", "On your phone, choose a file → Quick Share → Nearlane. You can minimize this window; the app will keep running in the system tray.") : !page.backend.account_authorized ? qsTranslate("Ui", "Connect the same Google account you use on your phone.") : page.backend.background_receiving ? qsTranslate("Ui", "Receiving will start once private keys are ready. If this takes too long, check the status in Settings.") : qsTranslate("Ui", "Enable receiving so your phone can find this computer.")
                    Layout.fillWidth: true
                }
                UI.ActionButton {
                    tokens: page.tokens
                    text: page.backend.receiving_active ? qsTranslate("Ui", "Stop receiving") : !page.backend.account_authorized ? qsTranslate("Ui", "Go to account") : qsTranslate("Ui", "Enable receiving")
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
                text: qsTranslate("Ui", "Accept this file?")
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
                    text: qsTranslate("Ui", "Accept")
                    primary: true
                    onClicked: page.backend.decide_incoming(true)
                }
                UI.ActionButton {
                    tokens: page.tokens
                    text: qsTranslate("Ui", "Decline")
                    onClicked: page.backend.decide_incoming(false)
                }
            }
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTranslate("Ui", "Download folder")
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
            text: qsTranslate("Ui", "Receiving settings")
            symbolName: "settings"
            onClicked: page.openSettings()
        }
        UI.BodyText {
            tokens: page.tokens
            text: qsTranslate("Ui", "Transfer confirmation")
            role: "title"
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            text: page.backend.confirm_each_file ? qsTranslate("Ui", "Each file will need your confirmation. You can accept or decline directly in the notification.") : qsTranslate("Ui", "Files from verified devices on your account are accepted automatically. A notification will appear once a file is saved.")
            Layout.fillWidth: true
        }
        UI.BodyText {
            tokens: page.tokens
            secondary: true
            role: "label"
            text: qsTranslate("Ui", "One file up to 64 GiB. Existing files are never overwritten.")
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
