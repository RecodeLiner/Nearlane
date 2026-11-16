import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import QtQuick.Dialogs
import Qt.labs.platform as Platform
import org.nearlane.app
import "components" as UI
import "pages" as Pages

ApplicationWindow {
    id: window
    width: 1040
    height: 760
    minimumWidth: 540
    minimumHeight: 540
    visible: true
    title: "Nearlane"
    color: theme.surface
    property int destination: 0
    readonly property bool wide: width >= 760
    property bool quitting: false
    property bool viewReady: false
    onDestinationChanged: { if (viewReady && !preview && !quitting) backendModel.set_send_surface(visible && destination === 0) }
    onVisibleChanged: { if (viewReady && !preview && !quitting) backendModel.set_send_surface(visible && destination === 0) }
    property int themeOverride: -1
    property real textScale: 1
    readonly property var preview: backendModel.design_preview.length > 0 ? JSON.parse(backendModel.design_preview) : null
    property bool incomingPending: backendModel.receive_pending
    property bool actionableNotification: backendModel.actionable_notification
    property int savedFiles: backendModel.received_count
    readonly property var destinations: [
        {
            label: qsTranslate("Ui", "Send"),
            title: qsTranslate("Ui", "Send a file"),
            icon: "send"
        },
        {
            label: qsTranslate("Ui", "Receive"),
            title: qsTranslate("Ui", "Receive files"),
            icon: "receive"
        },
        {
            label: qsTranslate("Ui", "Settings"),
            title: qsTranslate("Ui", "Settings"),
            icon: "settings"
        }
    ]
    Material.theme: themeOverride === 0 ? Material.Light : themeOverride === 1 ? Material.Dark : Material.System
    Material.primary: theme.primary
    Material.accent: theme.primary

    UI.Theme {
        id: theme
        dark: window.Material.theme === Material.Dark
        basePointSize: Math.max(12, window.font.pointSize) * window.textScale
        controlMaxWidth: Math.max(160, window.width - (window.wide ? 120 : 0) - 96)
    }
    Backend {
        id: backendModel
    }

    function showWindow() {
        window.showNormal();
        window.raise();
        window.requestActivate();
    }
    function showAccountSettings() {
        destination = 2;
        Qt.callLater(settingsPage.showAccount);
    }
    function showReceivingSettings() {
        destination = 2;
        Qt.callLater(settingsPage.showGeneral);
    }
    function quitApp() {
        quitting = true;
        backendModel.shutdown();
        Qt.quit();
    }
    onClosing: function (event) {
        if (!quitting && (!backendModel.smoke_test || backendModel.tray_test) && tray.available) {
            event.accepted = false;
            window.hide();
        } else {
            backendModel.shutdown();
            Qt.quit();
        }
    }
    function fallbackIncomingNotification() {
        if (incomingPending && !actionableNotification && !backendModel.smoke_test) {
            if (tray.available && tray.supportsMessages)
                tray.showMessage(qsTranslate("Ui", "Incoming file"), qsTranslate("Ui", "Your device wants to send a file. Open Nearlane to accept or decline."), Platform.SystemTrayIcon.Information, 10000);
            else {
                destination = 1;
                showWindow();
            }
        }
    }
    onIncomingPendingChanged: fallbackIncomingNotification()
    onActionableNotificationChanged: {
        if (!actionableNotification)
            fallbackIncomingNotification();
    }
    onSavedFilesChanged: {
        if (savedFiles > 0 && !backendModel.smoke_test && tray.available && tray.supportsMessages)
            tray.showMessage(qsTranslate("Ui", "File received"), qsTranslate("Ui", "The file was saved to the selected folder."), Platform.SystemTrayIcon.Information, 6000);
    }
    Platform.SystemTrayIcon {
        id: tray
        visible: !backendModel.smoke_test || backendModel.tray_test
        icon.source: "qrc:/nearlane/icon.svg"
        tooltip: backendModel.receiving_active ? qsTranslate("Ui", "Nearlane · Receiving from your devices") : qsTranslate("Ui", "Nearlane · Running in the background")
        onActivated: function (reason) {
            if (reason === Platform.SystemTrayIcon.Trigger || reason === Platform.SystemTrayIcon.DoubleClick)
                window.showWindow();
        }
        onMessageClicked: {
            window.destination = 1;
            window.showWindow();
        }
        menu: Platform.Menu {
            Platform.MenuItem {
                text: qsTranslate("Ui", "Open Nearlane")
                onTriggered: window.showWindow()
            }
            Platform.MenuItem {
                text: qsTranslate("Ui", "Receive from your devices")
                checkable: true
                checked: backendModel.background_receiving
                onTriggered: backendModel.configure_receiving(!backendModel.background_receiving, backendModel.confirm_each_file)
            }
            Platform.MenuItem {
                text: qsTranslate("Ui", "Find devices in the background")
                checkable: true
                checked: backendModel.continuous_discovery
                onTriggered: backendModel.set_background_discovery(!backendModel.continuous_discovery)
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTranslate("Ui", "Quit")
                onTriggered: window.quitApp()
            }
        }
    }
    Rectangle {
        id: appContent
        anchors.fill: parent
        color: theme.surface
        RowLayout {
            anchors.fill: parent
            spacing: 0
            Rectangle {
                visible: window.wide
                Layout.preferredWidth: 120
                Layout.fillHeight: true
                color: theme.surfaceLow
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 12
                    Image {
                        source: "qrc:/nearlane/icon.svg"
                        Layout.preferredWidth: 36
                        Layout.preferredHeight: 36
                        Layout.alignment: Qt.AlignHCenter
                        Layout.topMargin: 20
                        Layout.bottomMargin: 24
                        Accessible.ignored: true
                    }
                    Repeater {
                        id: railDestinations
                        model: window.destinations
                        UI.NavButton {
                            required property var modelData
                            required property int index
                            tokens: theme
                            text: modelData.label
                            symbolName: modelData.icon
                            selected: window.destination === index
                            Layout.fillWidth: true
                            Accessible.description: "Alt+" + (index + 1)
                            onClicked: window.destination = index
                        }
                    }
                    Item {
                        Layout.fillHeight: true
                    }
                    UI.BodyText {
                        tokens: theme
                        text: "Nearlane"
                        role: "label"
                        secondary: true
                        Layout.alignment: Qt.AlignHCenter
                        Layout.bottomMargin: 16
                    }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0
                Item {
                    Layout.fillWidth: true
                    implicitHeight: heading.implicitHeight + 48
                    ColumnLayout {
                        id: heading
                        anchors.fill: parent
                        anchors.margins: 24
                        spacing: 8
                        GridLayout {
                            columns: window.width < 660 && theme.basePointSize > 16 ? 1 : 2
                            Layout.fillWidth: true
                            UI.BodyText {
                                tokens: theme
                                text: window.destinations[window.destination].title
                                role: "headline"
                                Layout.fillWidth: true
                            }
                            UI.ActionButton {
                                tokens: theme
                                text: qsTranslate("Ui", "Account")
                                symbolName: "lock"
                                Layout.alignment: Qt.AlignRight
                                onClicked: window.showAccountSettings()
                            }
                        }
                        UI.BodyText {
                            tokens: theme
                            role: "label"
                            secondary: true
                            text: backendModel.receiving_active ? qsTranslate("Ui", "Receiving from your devices is enabled") : backendModel.account_busy ? qsTranslate("Ui", "Restoring sign-in…") : !backendModel.account_authorized ? qsTranslate("Ui", "Connect your account in Settings") : qsTranslate("Ui", "Receiving on this computer is off")
                            Layout.fillWidth: true
                        }
                    }
                }
                StackLayout {
                    id: pages
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: window.destination
                    Pages.SendPage {
                        id: sendPage
                        tokens: theme
                        backend: backendModel
                        onChooseFile: sendDialog.open()
                        onOpenSettings: window.showAccountSettings()
                    }
                    Pages.ReceivePage {
                        id: receivePage
                        tokens: theme
                        backend: backendModel
                        onOpenSettings: window.showReceivingSettings()
                        onOpenAccount: window.showAccountSettings()
                    }
                    Pages.SettingsPage {
                        id: settingsPage
                        tokens: theme
                        backend: backendModel
                        onChooseFolder: receiveFolder.open()
                        onChooseConfig: configDialog.open()
                    }
                }
                Rectangle {
                    visible: !window.wide
                    Layout.fillWidth: true
                    implicitHeight: compactNav.implicitHeight + 16
                    color: theme.surfaceLow
                    RowLayout {
                        id: compactNav
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 4
                        Repeater {
                            id: barDestinations
                            model: window.destinations
                            UI.NavButton {
                                required property var modelData
                                required property int index
                                tokens: theme
                                text: modelData.label
                                symbolName: modelData.icon
                                selected: window.destination === index
                                Layout.fillWidth: true
                                onClicked: window.destination = index
                            }
                        }
                    }
                }
            }
        }
    }
    Shortcut {
        sequence: "Alt+1"
        onActivated: window.destination = 0
    }
    Shortcut {
        sequence: "Alt+2"
        onActivated: window.destination = 1
    }
    Shortcut {
        sequence: "Alt+3"
        onActivated: window.destination = 2
    }
    Shortcut {
        sequences: [StandardKey.Open]
        enabled: backendModel.account_authorized && !backendModel.transfer_busy
        onActivated: {
            window.destination = 0;
            sendDialog.open();
        }
    }
    FileDialog {
        id: configDialog
        title: qsTranslate("Ui", "Sign-in configuration")
        nameFilters: [qsTranslate("Ui", "JSON files (*.json)")]
        onAccepted: backendModel.load_config(selectedFile)
    }
    FileDialog {
        id: sendDialog
        title: qsTranslate("Ui", "Choose a file")
        fileMode: FileDialog.OpenFile
        onAccepted: {
            sendPage.attemptedTransfer = false;
            backendModel.select_send_file(selectedFile);
        }
    }
    FolderDialog {
        id: receiveFolder
        title: qsTranslate("Ui", "Where to save received files")
        onAccepted: backendModel.select_receive_directory(selectedFolder)
    }
    Timer {
        interval: 100
        running: !window.preview
        repeat: true
        onTriggered: backendModel.poll_account()
    }

    // Explicit smoke-only fixtures: no real login, discovery, files or decisions.
    Component.onCompleted: {
        viewReady = true;
        if (!preview) {
            backendModel.set_send_surface(visible && destination === 0);
            return;
        }
        applyPreview();
    }
    function applyPreview() {
        width = preview.width;
        height = preview.height;
        themeOverride = preview.dark ? 1 : 0;
        textScale = preview.scale || 1;
        destination = preview.page || 0;
        backendModel.account_authorized = preview.scene !== "signed-out";
        backendModel.receiving_active = backendModel.account_authorized;
        backendModel.background_receiving = true;
        backendModel.discovery_available = backendModel.account_authorized;
        backendModel.selected_send_name = preview.fileName || "";
        backendModel.receive_directory = "/home/demo/Downloads/Nearlane";
        backendModel.own_devices = preview.devices || [];
        sendPage.selectedDevice = backendModel.own_devices.length > 0 ? 0 : -1;
        backendModel.discovery_busy = preview.scene === "loading";
        backendModel.account_status = preview.scene === "signed-out" ? qsTranslate("Ui", "Sign in to connect your devices.") : preview.scene === "error" ? qsTranslate("Ui", "Could not restore sign-in. Retry saved sign-in.") : qsTranslate("Ui", "Quick Share permission granted.");
        backendModel.preferences_status = preview.scene === "error" ? qsTranslate("Ui", "Could not save settings. Check access to the configuration folder.") : "";
        backendModel.incoming_offer = preview.offer || "";
        backendModel.receive_pending = preview.scene === "incoming";
        backendModel.receive_status = preview.scene === "error" ? qsTranslate("Ui", "Could not open the receiving port. Close another Quick Share receiver and retry.") : qsTranslate("Ui", "Ready to receive from your own devices.");
        backendModel.confirm_each_file = preview.scene === "incoming";
        backendModel.continuous_discovery = true;
    }
    Timer {
        interval: 650
        running: !!window.preview
        onTriggered: {
            if (preview.exerciseLocalization) {
                const original = backendModel.language;
                for (const language of ["ru", "en"]) {
                    backendModel.select_language(language);
                    const label = language === "ru" ? "Отправить" : "Send";
                    if (window.destinations[0].label !== label || backendModel.effective_language !== language)
                        throw new Error("Live QML translation failed: " + language);
                    if (!backendModel.status.startsWith(language === "ru" ? "Платформа:" : "Platform:"))
                        throw new Error("Deferred backend translation failed: " + language);
                }
                backendModel.select_language("invalid-language");
                if (backendModel.language !== "en")
                    throw new Error("Invalid language was accepted");
                backendModel.select_language(original);
                window.applyPreview();
                console.log("preview: live language switching checked");
            }
            if (preview.exerciseNavigation) {
                const selected = sendPage.selectedDevice;
                const initialWidth = width;
                railDestinations.itemAt(2).clicked();
                if (destination !== 2)
                    throw new Error("Rail navigation failed");
                width = 540;
                Qt.callLater(function () {
                    if (window.wide || destination !== 2)
                        throw new Error("Compact navigation lost destination");
                    barDestinations.itemAt(0).clicked();
                    if (destination !== 0 || sendPage.selectedDevice !== selected)
                        throw new Error("Selection was lost on resize");
                    width = initialWidth;
                    Qt.callLater(function () {
                        if (!window.wide || !sendPage.focusFilePicker())
                            throw new Error("Keyboard focus or expanded layout failed");
                        console.log("preview: navigation, resize continuity and Tab focus checked");
                        window.capturePreview();
                    });
                });
            } else
                window.capturePreview();
        }
    }
    function capturePreview() {
        if (preview.scrollBottom) {
            const view = [sendPage, receivePage, settingsPage][destination];
            view.contentItem.contentY = Math.max(0, view.contentItem.contentHeight - view.availableHeight);
        }
        if (!appContent.grabToImage(function (result) {
            if (!result.saveToFile(window.preview.output))
                throw new Error("Could not save preview");
            console.log("preview: saved synthetic UI");
            Qt.quit();
        }))
            throw new Error("Could not capture preview");
    }
    Timer {
        interval: 800
        running: backendModel.smoke_test && !window.preview
        onTriggered: {
            backendModel.check_contacts();
            const savedLanguage = backendModel.language;
            backendModel.select_language("ru");
            if (window.destinations[0].label !== "Отправить")
                throw new Error("Russian translation did not load");
            backendModel.select_language("en");
            if (window.destinations[0].label !== "Send")
                throw new Error("English translation did not load");
            backendModel.select_language(savedLanguage);
            console.log("smoke: live translations checked");
            if (backendModel.tray_test) {
                if (!tray.available)
                    throw new Error("System tray is not available");
                if (!backendModel.receive_directory.endsWith("/Nearlane") && !backendModel.receive_directory.endsWith("\\Nearlane"))
                    throw new Error("Default receiving folder is missing");
                window.close();
                if (window.visible)
                    throw new Error("Closing did not hide to tray");
                window.showWindow();
                if (!window.visible)
                    throw new Error("Window did not restore from tray");
                if (tray.supportsMessages)
                    tray.showMessage("Nearlane", "Background notification check", Platform.SystemTrayIcon.Information, 2500);
                console.log("smoke: tray close/show and notification checked");
            }
            Qt.quit();
        }
    }
}
