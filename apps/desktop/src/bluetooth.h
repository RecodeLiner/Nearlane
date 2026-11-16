// Original Nearlane adapter (MIT OR Apache-2.0). Qt selects BlueZ/CoreBluetooth.
#pragma once
#include <QBluetoothDeviceDiscoveryAgent>
#include <QBluetoothDeviceInfo>
#include <QBluetoothUuid>
#include <QCoreApplication>
#include <QElapsedTimer>
#include <QFile>
#include <QPermissions>
#include <QSet>
#include <QStringList>
#include <QTimer>
#include <memory>

namespace nearlane::desktop {
class BluetoothDiscovery : public QObject {
    std::unique_ptr<QBluetoothDeviceDiscoveryAgent> agent;
    QTimer restart;
    QElapsedTimer epoch;
    QSet<QByteArray> seen;
    QStringList pending;
    bool wanted = false;
    int state = 0;
    unsigned processed = 0;
    void observe(const QBluetoothDeviceInfo &device) {
        if (!wanted) return;
        if (epoch.elapsed() >= 60000) { epoch.restart(); processed = 0; seen.clear(); }
        if (++processed > 8192) return;
        const QBluetoothUuid service{QUuid{"0000fef3-0000-1000-8000-00805f9b34fb"}};
        auto data = device.serviceData(service);
        // Google's Apple adapter encodes service data in a local name when
        // CoreBluetooth cannot advertise ServiceData. Never display that name.
        if (data.isEmpty() && device.serviceUuids().contains(service)) {
            const auto name = device.name().toUtf8();
            if (name.size() <= 44)
                data = QByteArray::fromBase64(name, QByteArray::Base64UrlEncoding | QByteArray::AbortOnBase64DecodingErrors);
        }
        if ((data.size() != 25 && data.size() != 27) || seen.contains(data) || pending.size() >= 64 || seen.size() >= 512) return;
        seen.insert(data);
        pending.append(QString::fromLatin1(data.toHex()));
    }
    void begin() {
        if (!wanted) return;
#if defined(Q_OS_MACOS)
        QFile info(QCoreApplication::applicationDirPath() + "/../Info.plist");
        if (!info.open(QIODevice::ReadOnly) || !info.readAll().contains("NSBluetoothAlwaysUsageDescription")) {
            state = 6; return; // Run the packaged app, do not crash on missing TCC metadata.
        }
#endif
        QBluetoothPermission permission;
        permission.setCommunicationModes(QBluetoothPermission::Access);
        const auto access = qApp->checkPermission(permission);
        if (access == Qt::PermissionStatus::Undetermined) {
            state = 1;
            qApp->requestPermission(permission, this, [this](const QPermission &result) {
                if (!wanted) return;
                if (result.status() == Qt::PermissionStatus::Granted) begin(); else state = 4;
            });
            return;
        }
        if (access != Qt::PermissionStatus::Granted) { state = 4; return; }
        if (!agent) {
            agent = std::make_unique<QBluetoothDeviceDiscoveryAgent>();
            agent->setLowEnergyDiscoveryTimeout(15000);
            connect(agent.get(), &QBluetoothDeviceDiscoveryAgent::deviceDiscovered, this, [this](const QBluetoothDeviceInfo &d) { observe(d); });
            connect(agent.get(), &QBluetoothDeviceDiscoveryAgent::deviceUpdated, this, [this](const QBluetoothDeviceInfo &d, QBluetoothDeviceInfo::Fields) { observe(d); });
            connect(agent.get(), &QBluetoothDeviceDiscoveryAgent::finished, this, [this] { if (wanted) restart.start(2000); });
            connect(agent.get(), &QBluetoothDeviceDiscoveryAgent::errorOccurred, this, [this](QBluetoothDeviceDiscoveryAgent::Error error) {
                state = error == QBluetoothDeviceDiscoveryAgent::PoweredOffError ? 3 : error == QBluetoothDeviceDiscoveryAgent::MissingPermissionsError ? 4 : 5;
                if (wanted && state == 3) restart.start(15000);
            });
        }
        if (!(QBluetoothDeviceDiscoveryAgent::supportedDiscoveryMethods() & QBluetoothDeviceDiscoveryAgent::LowEnergyMethod)) { state = 5; return; }
        state = 2;
        agent->start(QBluetoothDeviceDiscoveryAgent::LowEnergyMethod);
    }
public:
    BluetoothDiscovery() {
        epoch.start(); restart.setSingleShot(true);
        connect(&restart, &QTimer::timeout, this, [this] { begin(); });
    }
    ~BluetoothDiscovery() override { set_scanning(false); }
    void set_scanning(bool enable) {
        if (wanted == enable) return;
        wanted = enable;
        if (enable) begin();
        else { restart.stop(); if (agent) agent->stop(); agent.reset(); pending.clear(); seen.clear(); state = 0; }
    }
    int scan_status() const { return state; }
    QStringList drain_advertisements() { auto result = pending; pending.clear(); return result; }
};
inline std::unique_ptr<BluetoothDiscovery> create_bluetooth_discovery() { return std::make_unique<BluetoothDiscovery>(); }
}
