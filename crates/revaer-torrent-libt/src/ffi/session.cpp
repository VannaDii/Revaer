#include "revaer/session.hpp"

#include "revaer-torrent-libt/src/ffi/bridge.rs.h"

#include <algorithm>
#include <array>
#include <cctype>
#include <chrono>
#include <filesystem>
#include <optional>
#include <regex>
#include <string>
#include <unordered_set>
#include <utility>
#include <vector>
#include <iterator>

#if defined(__clang__)
#define REVAER_SUPPRESS_DEPRECATED_BEGIN \
    _Pragma("clang diagnostic push") \
    _Pragma("clang diagnostic ignored \"-Wdeprecated-declarations\"")
#define REVAER_SUPPRESS_DEPRECATED_END _Pragma("clang diagnostic pop")
#elif defined(__GNUC__)
#define REVAER_SUPPRESS_DEPRECATED_BEGIN \
    _Pragma("GCC diagnostic push") \
    _Pragma("GCC diagnostic ignored \"-Wdeprecated-declarations\"")
#define REVAER_SUPPRESS_DEPRECATED_END _Pragma("GCC diagnostic pop")
#else
#define REVAER_SUPPRESS_DEPRECATED_BEGIN
#define REVAER_SUPPRESS_DEPRECATED_END
#endif

#include <libtorrent/add_torrent_params.hpp>
#include <libtorrent/alert.hpp>
#include <libtorrent/alert_types.hpp>
#include <libtorrent/download_priority.hpp>
#include <libtorrent/error_code.hpp>
#include <libtorrent/address.hpp>
#include <libtorrent/ip_filter.hpp>
#include <libtorrent/file_storage.hpp>
#include <libtorrent/info_hash.hpp>
#include <libtorrent/magnet_uri.hpp>
#include <libtorrent/bencode.hpp>
#include <libtorrent/session.hpp>
#include <libtorrent/session_params.hpp>
#include <libtorrent/settings_pack.hpp>
#include <libtorrent/session_types.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_info.hpp>
#include <libtorrent/torrent_flags.hpp>
#include <libtorrent/torrent_status.hpp>
#include <libtorrent/write_resume_data.hpp>

namespace revaer {

namespace {

constexpr std::array<const char*, 5> kSkipFluffPatterns = {
    "**/sample/**",
    "**/samples/**",
    "**/extras/**",
    "**/proof/**",
    "**/screens/**",
};

std::string to_std_string(::rust::Str value) {
    return std::string(value.data(), value.length());
}

std::string to_std_string(const ::rust::String& value) {
    return static_cast<std::string>(value);
}

std::string glob_to_regex(const std::string& pattern) {
    std::string regex;
    regex.reserve(pattern.size() * 2);
    regex.push_back('^');
    for (char ch : pattern) {
        switch (ch) {
            case '*':
                regex.append(".*");
                break;
            case '?':
                regex.push_back('.');
                break;
            case '.':
            case '^':
            case '$':
            case '|':
            case '(':
            case ')':
            case '[':
            case ']':
            case '{':
            case '}':
            case '+':
            case '\\':
                regex.push_back('\\');
                regex.push_back(ch);
                break;
            default:
                regex.push_back(ch);
                break;
        }
    }
    regex.push_back('$');
    return regex;
}

NativeTorrentState map_state(lt::torrent_status::state_t state) {
    using ts = lt::torrent_status;
    switch (state) {
        case ts::state_t::checking_files:
        case ts::state_t::checking_resume_data:
            return NativeTorrentState::Queued;
        case ts::state_t::downloading_metadata:
            return NativeTorrentState::FetchingMetadata;
        case ts::state_t::downloading:
            return NativeTorrentState::Downloading;
        case ts::state_t::finished:
            return NativeTorrentState::Completed;
        case ts::state_t::seeding:
            return NativeTorrentState::Seeding;
        default:
            return NativeTorrentState::Stopped;
    }
}

lt::download_priority_t to_priority(std::uint8_t value) {
    switch (value) {
        case 0:
            return lt::dont_download;
        case 1:
            return lt::low_priority;
        case 7:
            return lt::top_priority;
        default:
            return lt::download_priority_t{value};
    }
}

struct SelectionEntry {
    std::vector<std::regex> include;
    std::vector<std::regex> exclude;
    std::vector<FilePriorityOverride> overrides;
    bool skip_fluff{false};
};

struct TorrentSnapshot {
    NativeTorrentState state{NativeTorrentState::Queued};
    std::uint64_t bytes_downloaded{0};
    std::uint64_t bytes_total{0};
    bool metadata_applied{false};
    bool metadata_emitted{false};
    bool completed_emitted{false};
    std::string last_name;
    std::string last_download_dir;
};
}  // namespace

class Session::Impl {
public:
    explicit Impl(const SessionOptions& options) {
        lt::settings_pack pack;
        pack.set_bool(lt::settings_pack::enable_dht, options.enable_dht);
        pack.set_bool(lt::settings_pack::enable_lsd, false);
        pack.set_bool(lt::settings_pack::enable_upnp, false);
        pack.set_bool(lt::settings_pack::enable_natpmp, false);
        pack.set_bool(lt::settings_pack::enable_outgoing_utp, false);
        pack.set_bool(lt::settings_pack::enable_incoming_utp, false);
        pack.set_bool(lt::settings_pack::anonymous_mode, false);
        REVAER_SUPPRESS_DEPRECATED_BEGIN
        pack.set_bool(lt::settings_pack::force_proxy, false);
        REVAER_SUPPRESS_DEPRECATED_END
        pack.set_bool(lt::settings_pack::prefer_rc4, false);
        pack.set_bool(lt::settings_pack::allow_multiple_connections_per_ip, false);
        pack.set_int(lt::settings_pack::alert_mask,
                     lt::alert_category::status | lt::alert_category::error |
                         lt::alert_category::storage | lt::alert_category::file_progress);

        lt::session_params params(pack);
        session_ = std::make_unique<lt::session>(params);
        default_download_root_ = to_std_string(options.download_root);
        resume_dir_ = to_std_string(options.resume_dir);
        sequential_default_ = options.sequential_default;

        if (!resume_dir_.empty()) {
            std::error_code ec;
            std::filesystem::create_directories(resume_dir_, ec);
        }
    }

    ::rust::String apply_engine_profile(const EngineOptions& options) {
        try {
            lt::settings_pack pack;
            pack.set_bool(lt::settings_pack::enable_dht, options.network.enable_dht);
            pack.set_bool(lt::settings_pack::enable_lsd, options.network.enable_lsd);
            pack.set_bool(lt::settings_pack::enable_upnp, options.network.enable_upnp);
            pack.set_bool(lt::settings_pack::enable_natpmp, options.network.enable_natpmp);
            pack.set_bool(lt::settings_pack::enable_outgoing_utp,
                          options.network.enable_outgoing_utp);
            pack.set_bool(lt::settings_pack::enable_incoming_utp,
                          options.network.enable_incoming_utp);
            pack.set_bool(lt::settings_pack::anonymous_mode, options.network.anonymous_mode);
            REVAER_SUPPRESS_DEPRECATED_BEGIN
            pack.set_bool(lt::settings_pack::force_proxy, options.network.force_proxy);
            REVAER_SUPPRESS_DEPRECATED_END
            pack.set_bool(lt::settings_pack::prefer_rc4, options.network.prefer_rc4);
            pack.set_bool(lt::settings_pack::allow_multiple_connections_per_ip,
                          options.network.allow_multiple_connections_per_ip);

            if (options.network.has_listen_interfaces &&
                !options.network.listen_interfaces.empty()) {
                std::string combined;
                for (std::size_t i = 0; i < options.network.listen_interfaces.size(); ++i) {
                    if (i > 0) {
                        combined.push_back(',');
                    }
                    combined.append(to_std_string(options.network.listen_interfaces[i]));
                }
                pack.set_str(lt::settings_pack::listen_interfaces, combined);
                pack.set_int(lt::settings_pack::max_retry_port_bind, 0);
            } else if (options.network.set_listen_port && options.network.listen_port > 0) {
                pack.set_str(lt::settings_pack::listen_interfaces,
                             "0.0.0.0:" + std::to_string(options.network.listen_port));
                pack.set_int(lt::settings_pack::max_retry_port_bind, 0);
            } else if (options.tracker.has_listen_interface) {
                pack.set_int(lt::settings_pack::max_retry_port_bind, 0);
                pack.set_str(lt::settings_pack::listen_interfaces,
                             to_std_string(options.tracker.listen_interface));
            }

            std::vector<std::string> dht_nodes;
            dht_nodes.reserve(options.network.dht_bootstrap_nodes.size() +
                              options.network.dht_router_nodes.size());
            std::unordered_set<std::string> seen;
            auto append_nodes = [&](const ::rust::Vec<::rust::String>& nodes) {
                for (const auto& node : nodes) {
                    auto normalized = to_std_string(node);
                    if (normalized.empty()) {
                        continue;
                    }
                    std::string key = normalized;
                    std::transform(key.begin(), key.end(), key.begin(), [](unsigned char ch) {
                        return static_cast<char>(std::tolower(ch));
                    });
                    if (seen.insert(key).second) {
                        dht_nodes.push_back(std::move(normalized));
                    }
                }
            };
            append_nodes(options.network.dht_bootstrap_nodes);
            append_nodes(options.network.dht_router_nodes);

            if (!dht_nodes.empty()) {
                std::string combined = dht_nodes.front();
                for (std::size_t i = 1; i < dht_nodes.size(); ++i) {
                    combined.append(",").append(dht_nodes[i]);
                }
                pack.set_str(lt::settings_pack::dht_bootstrap_nodes, combined);
            } else {
                pack.set_str(lt::settings_pack::dht_bootstrap_nodes, "");
            }

            if (options.limits.max_active > 0) {
                pack.set_int(lt::settings_pack::active_downloads, options.limits.max_active);
                pack.set_int(lt::settings_pack::active_limit, options.limits.max_active);
            }

            pack.set_int(lt::settings_pack::out_enc_policy, options.network.encryption_policy);
            pack.set_int(lt::settings_pack::in_enc_policy, options.network.encryption_policy);

            if (!options.storage.download_root.empty()) {
                default_download_root_ = to_std_string(options.storage.download_root);
            }
            if (!options.storage.resume_dir.empty()) {
                const auto resume_dir = to_std_string(options.storage.resume_dir);
                if (resume_dir != resume_dir_) {
                    resume_dir_ = resume_dir;
                    std::error_code ec;
                    std::filesystem::create_directories(resume_dir_, ec);
                }
            }

            sequential_default_ = options.behavior.sequential_default;

            pack.set_int(
                lt::settings_pack::download_rate_limit,
                options.limits.download_rate_limit >= 0
                    ? static_cast<int>(options.limits.download_rate_limit)
                    : -1);
            pack.set_int(lt::settings_pack::upload_rate_limit,
                         options.limits.upload_rate_limit >= 0
                             ? static_cast<int>(options.limits.upload_rate_limit)
                             : -1);

            if (options.tracker.has_user_agent) {
                pack.set_str(lt::settings_pack::user_agent,
                             to_std_string(options.tracker.user_agent));
            }
            if (options.tracker.has_announce_ip) {
                pack.set_str(lt::settings_pack::announce_ip,
                             to_std_string(options.tracker.announce_ip));
            }
            if (options.tracker.has_listen_interface) {
                pack.set_str(lt::settings_pack::listen_interfaces,
                             to_std_string(options.tracker.listen_interface));
            }
            if (options.tracker.has_request_timeout) {
                const auto seconds =
                    std::max<std::int64_t>(1, options.tracker.request_timeout_ms / 1000);
                pack.set_int(lt::settings_pack::request_timeout,
                             static_cast<int>(seconds));
            }
            pack.set_bool(lt::settings_pack::announce_to_all_trackers,
                          options.tracker.announce_to_all);

            announce_to_all_ = options.tracker.announce_to_all;
            default_trackers_.clear();
            default_trackers_.reserve(options.tracker.default_trackers.size());
            for (const auto& tracker : options.tracker.default_trackers) {
                default_trackers_.push_back(to_std_string(tracker));
            }
            extra_trackers_.clear();
            extra_trackers_.reserve(options.tracker.extra_trackers.size());
            for (const auto& tracker : options.tracker.extra_trackers) {
                extra_trackers_.push_back(to_std_string(tracker));
            }
            replace_default_trackers_ = options.tracker.replace_trackers;

            if (options.tracker.proxy.has_proxy) {
                pack.set_str(lt::settings_pack::proxy_hostname,
                             to_std_string(options.tracker.proxy.host));
                pack.set_int(lt::settings_pack::proxy_port, options.tracker.proxy.port);
                pack.set_bool(lt::settings_pack::proxy_peer_connections,
                              options.tracker.proxy.proxy_peers);
                int proxy_type = lt::settings_pack::http;
                switch (options.tracker.proxy.kind) {
                    case 0:
                        proxy_type = lt::settings_pack::http;
                        break;
                    case 1:
                        proxy_type = lt::settings_pack::http;
                        break;
                    case 2:
                        proxy_type = lt::settings_pack::socks5;
                        break;
                    default:
                        proxy_type = lt::settings_pack::http;
                        break;
                }
                pack.set_int(lt::settings_pack::proxy_type, proxy_type);
            } else {
                pack.set_int(lt::settings_pack::proxy_type, lt::settings_pack::none);
            }

            if (options.network.has_ip_filter) {
                lt::ip_filter filter;
                for (const auto& rule : options.network.ip_filter_rules) {
                    lt::error_code ec;
                    const auto start = lt::make_address(to_std_string(rule.start), ec);
                    if (ec) {
                        return ::rust::String(ec.message());
                    }
                    const auto end = lt::make_address(to_std_string(rule.end), ec);
                    if (ec) {
                        return ::rust::String(ec.message());
                    }
                    filter.add_rule(start, end, lt::ip_filter::blocked);
                }
                session_->set_ip_filter(filter);
            } else {
                session_->set_ip_filter(lt::ip_filter{});
            }

            session_->apply_settings(pack);
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    ::rust::String add_torrent(const AddTorrentRequest& request) {
        try {
            lt::add_torrent_params params;
            const auto request_id = to_std_string(request.id);
            const auto download_dir = to_std_string(request.download_dir);
            params.save_path = request.has_download_dir ? download_dir : default_download_root_;
            if (params.save_path.empty()) {
                return "download directory not configured";
            }

            if (request.source_kind == SourceKind::Magnet) {
                params = lt::parse_magnet_uri(to_std_string(request.magnet_uri));
                params.save_path =
                    request.has_download_dir ? download_dir : default_download_root_;
            } else {
                if (request.metainfo.empty()) {
                    return "metainfo payload empty";
                }
                lt::span<const char> buffer(
                    reinterpret_cast<const char*>(request.metainfo.data()),
                    static_cast<long>(request.metainfo.size()));
                params.ti = std::make_shared<lt::torrent_info>(buffer);
            }

            auto resume_it = pending_resume_.find(request_id);
            if (resume_it != pending_resume_.end()) {
                REVAER_SUPPRESS_DEPRECATED_BEGIN
                params.resume_data = resume_it->second;
                REVAER_SUPPRESS_DEPRECATED_END
                pending_resume_.erase(resume_it);
            }

            params.flags |= lt::torrent_flags::auto_managed;
            params.flags &= ~lt::torrent_flags::seed_mode;

            lt::torrent_handle handle = session_->add_torrent(params);
            handles_[request_id] = handle;
            snapshots_[request_id] = TorrentSnapshot{};

            bool sequential = sequential_default_;
            if (request.has_sequential_override) {
                sequential = request.sequential;
            }
            if (sequential) {
                handle.set_flags(lt::torrent_flags::sequential_download);
            } else {
                handle.unset_flags(lt::torrent_flags::sequential_download);
            }

            std::vector<std::string> trackers;
            if (!replace_default_trackers_) {
                trackers.insert(trackers.end(), default_trackers_.begin(), default_trackers_.end());
                trackers.insert(trackers.end(), extra_trackers_.begin(), extra_trackers_.end());
            }
            if (request.replace_trackers) {
                trackers.clear();
                trackers.reserve(request.trackers.size());
                for (const auto& tracker : request.trackers) {
                    trackers.push_back(to_std_string(tracker));
                }
            } else {
                for (const auto& tracker : request.trackers) {
                    trackers.push_back(to_std_string(tracker));
                }
            }
            if (!trackers.empty()) {
                params.trackers = trackers;
            }

            (void)request.tags;
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    ::rust::String remove_torrent(::rust::Str id, bool with_data) {
        const std::string key = to_std_string(id);
        auto it = handles_.find(key);
        if (it == handles_.end()) {
            return ::rust::String();
        }
        try {
            lt::remove_flags_t flags = {};
            if (with_data) {
                flags = lt::session::delete_files;
            }
            session_->remove_torrent(it->second, flags);
            handles_.erase(it);
            snapshots_.erase(key);
            selection_rules_.erase(key);
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    ::rust::String pause_torrent(::rust::Str id) {
        return mutate_handle(to_std_string(id), [](lt::torrent_handle& handle) {
            handle.unset_flags(lt::torrent_flags::auto_managed);
            handle.pause();
        });
    }

    ::rust::String resume_torrent(::rust::Str id) {
        return mutate_handle(to_std_string(id), [](lt::torrent_handle& handle) {
            handle.set_flags(lt::torrent_flags::auto_managed);
            handle.resume();
        });
    }

    ::rust::String set_sequential(::rust::Str id, bool sequential) {
        return mutate_handle(to_std_string(id), [sequential](lt::torrent_handle& handle) {
            if (sequential) {
                handle.set_flags(lt::torrent_flags::sequential_download);
            } else {
                handle.unset_flags(lt::torrent_flags::sequential_download);
            }
        });
    }

    ::rust::String load_fastresume(::rust::Str id,
                                   rust::Slice<const std::uint8_t> data) {
        std::vector<char> buffer;
        buffer.resize(data.size());
        std::copy(data.begin(), data.end(), buffer.begin());
        pending_resume_[to_std_string(id)] = std::move(buffer);
        return ::rust::String();
    }

    ::rust::String update_limits(const LimitRequest& request) {
        try {
            if (request.apply_globally) {
                lt::settings_pack pack;
                pack.set_int(lt::settings_pack::download_rate_limit,
                             request.download_bps >= 0
                                 ? static_cast<int>(request.download_bps)
                                 : -1);
                pack.set_int(lt::settings_pack::upload_rate_limit,
                             request.upload_bps >= 0
                                 ? static_cast<int>(request.upload_bps)
                                 : -1);
                session_->apply_settings(pack);
            } else {
                const auto key = to_std_string(request.id);
                auto it = handles_.find(key);
                if (it == handles_.end()) {
                    return ::rust::String();
                }
                if (request.download_bps >= 0) {
                    it->second.set_download_limit(static_cast<int>(request.download_bps));
                } else {
                    it->second.set_download_limit(-1);
                }
                if (request.upload_bps >= 0) {
                    it->second.set_upload_limit(static_cast<int>(request.upload_bps));
                } else {
                    it->second.set_upload_limit(-1);
                }
            }
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    ::rust::String update_selection(const SelectionRules& rules) {
        SelectionEntry entry;
        entry.skip_fluff = rules.skip_fluff;
        entry.overrides.assign(rules.priorities.begin(), rules.priorities.end());

        entry.include.reserve(rules.include.size());
        for (const auto& pattern : rules.include) {
            entry.include.emplace_back(glob_to_regex(to_std_string(pattern)), std::regex::icase);
        }

        entry.exclude.reserve(rules.exclude.size());
        for (const auto& pattern : rules.exclude) {
            entry.exclude.emplace_back(glob_to_regex(to_std_string(pattern)), std::regex::icase);
        }

        const auto key = to_std_string(rules.id);
        selection_rules_[key] = std::move(entry);

        auto it = handles_.find(key);
        if (it != handles_.end()) {
            apply_selection(it->first, it->second);
        }
        return ::rust::String();
    }

    ::rust::String reannounce(::rust::Str id) {
        return mutate_handle(to_std_string(id), [](lt::torrent_handle& handle) {
            handle.force_reannounce();
        });
    }

    ::rust::String recheck(::rust::Str id) {
        return mutate_handle(to_std_string(id), [](lt::torrent_handle& handle) {
            handle.force_recheck();
        });
    }

    rust::Vec<NativeEvent> poll_events() {
        rust::Vec<NativeEvent> events;

        std::vector<lt::alert*> alerts;
        session_->pop_alerts(&alerts);
        for (lt::alert* alert : alerts) {
            if (auto* err = lt::alert_cast<lt::torrent_error_alert>(alert)) {
                auto id = find_torrent_id(err->handle);
                if (!id.empty()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::Error;
                    evt.state = NativeTorrentState::Failed;
                    evt.message = err->error.message();
                    events.push_back(evt);
                }
            }
        }

        for (auto& [id, handle] : handles_) {
            lt::torrent_status status = handle.status(
                lt::torrent_handle::query_name | lt::torrent_handle::query_save_path |
                lt::torrent_handle::query_pieces | lt::torrent_handle::query_torrent_file);

            auto& snapshot = snapshots_[id];
            NativeTorrentState current_state = map_state(status.state);

            if (status.errc) {
                NativeEvent evt{};
                evt.id = id;
                evt.kind = NativeEventKind::Error;
                evt.state = NativeTorrentState::Failed;
                evt.message = status.errc.message();
                events.push_back(evt);
            }

            if (!snapshot.metadata_emitted) {
                auto info = handle.torrent_file();
                if (info) {
                    NativeEvent files_evt{};
                    files_evt.id = id;
                    files_evt.kind = NativeEventKind::FilesDiscovered;
                    files_evt.state = current_state;
                    files_evt.name = info->name();
                    files_evt.download_dir = status.save_path;
                    files_evt.files = rust::Vec<NativeFile>();
                    for (lt::file_index_t idx : info->files().file_range()) {
                        NativeFile file{};
                        file.index = static_cast<std::uint32_t>(static_cast<int>(idx));
                        file.path = info->files().file_path(idx);
                        file.size_bytes = static_cast<std::uint64_t>(info->files().file_size(idx));
                        files_evt.files.push_back(std::move(file));
                    }
                    events.push_back(files_evt);

                    apply_selection(id, handle);
                    snapshot.metadata_applied = true;
                    snapshot.last_name = info->name();
                    snapshot.last_download_dir = status.save_path;
                    snapshot.metadata_emitted = true;
                }
            }

            if (snapshot.last_name != status.name || snapshot.last_download_dir != status.save_path) {
                NativeEvent meta{};
                meta.id = id;
                meta.kind = NativeEventKind::MetadataUpdated;
                meta.state = current_state;
                meta.name = status.name;
                meta.download_dir = status.save_path;
                events.push_back(meta);
                snapshot.last_name = status.name;
                snapshot.last_download_dir = status.save_path;
            }

            if (snapshot.state != current_state) {
                NativeEvent state_evt{};
                state_evt.id = id;
                state_evt.kind = NativeEventKind::StateChanged;
                state_evt.state = current_state;
                state_evt.name = status.name;
                state_evt.download_dir = status.save_path;
                events.push_back(state_evt);
                snapshot.state = current_state;
            }

            if (static_cast<std::uint64_t>(status.total_done) != snapshot.bytes_downloaded ||
                static_cast<std::uint64_t>(status.total_wanted) != snapshot.bytes_total) {
                NativeEvent progress{};
                progress.id = id;
                progress.kind = NativeEventKind::Progress;
                progress.state = current_state;
                progress.name = status.name;
                progress.download_dir = status.save_path;
                progress.bytes_downloaded = static_cast<std::uint64_t>(status.total_done);
                progress.bytes_total = static_cast<std::uint64_t>(status.total_wanted);
                progress.download_bps = static_cast<std::uint64_t>(
                    status.download_payload_rate > 0 ? status.download_payload_rate : 0);
                progress.upload_bps = static_cast<std::uint64_t>(
                    status.upload_payload_rate > 0 ? status.upload_payload_rate : 0);
                if (status.total_payload_download > 0) {
                    progress.ratio = static_cast<double>(status.total_payload_upload) /
                                     static_cast<double>(status.total_payload_download);
                } else {
                    progress.ratio = 0.0;
                }
                events.push_back(progress);

                snapshot.bytes_downloaded = static_cast<std::uint64_t>(status.total_done);
                snapshot.bytes_total = static_cast<std::uint64_t>(status.total_wanted);
            }

            if (!snapshot.completed_emitted &&
                (status.is_finished || status.state == lt::torrent_status::seeding)) {
                NativeEvent completed{};
                completed.id = id;
                completed.kind = NativeEventKind::Completed;
                completed.state = NativeTorrentState::Completed;
                completed.name = status.name;
                completed.library_path = status.save_path;
                events.push_back(completed);
                snapshot.completed_emitted = true;
            }

            if (status.need_save_resume) {
                try {
                    REVAER_SUPPRESS_DEPRECATED_BEGIN
                    auto resume_entry = handle.write_resume_data();
                    REVAER_SUPPRESS_DEPRECATED_END
                    std::vector<char> buffer;
                    lt::bencode(std::back_inserter(buffer), resume_entry);
                    NativeEvent resume{};
                    resume.id = id;
                    resume.kind = NativeEventKind::ResumeData;
                    resume.state = current_state;
                    resume.resume_data = rust::Vec<std::uint8_t>();
                    resume.resume_data.reserve(static_cast<std::size_t>(buffer.size()));
                    for (auto byte : buffer) {
                        resume.resume_data.push_back(static_cast<std::uint8_t>(byte));
                    }
                    events.push_back(resume);
                } catch (const std::exception&) {
                    // ignore failures
                }
            }
        }

        return events;
    }

private:
    template <typename Fn>
    ::rust::String mutate_handle(const std::string& id, Fn&& fn) {
        auto it = handles_.find(id);
        if (it == handles_.end()) {
            return ::rust::String();
        }
        try {
            fn(it->second);
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    std::string find_torrent_id(const lt::torrent_handle& handle) const {
        for (const auto& [id, stored] : handles_) {
            if (stored == handle) {
                return id;
            }
        }
        return {};
    }

    bool matches_any(const std::vector<std::regex>& patterns, const std::string& value) const {
        return std::any_of(patterns.begin(), patterns.end(),
                           [&value](const std::regex& re) {
                               return std::regex_match(value, re);
                           });
    }

    void apply_selection(const std::string& id, lt::torrent_handle& handle) {
        auto info = handle.torrent_file();
        if (!info) {
            return;
        }
        auto rules_it = selection_rules_.find(id);
        if (rules_it == selection_rules_.end()) {
            return;
        }

        const SelectionEntry& rules = rules_it->second;

        std::vector<lt::download_priority_t> priorities;
        priorities.resize(static_cast<std::size_t>(info->files().num_files()),
                          lt::default_priority);

        for (lt::file_index_t idx : info->files().file_range()) {
            std::string path = info->files().file_path(idx);

            if (rules.skip_fluff && is_fluff(path)) {
                priorities[static_cast<std::size_t>(idx)] = lt::dont_download;
                continue;
            }

            if (!rules.exclude.empty() && matches_any(rules.exclude, path)) {
                priorities[static_cast<std::size_t>(idx)] = lt::dont_download;
                continue;
            }

            if (!rules.include.empty() && matches_any(rules.include, path)) {
                priorities[static_cast<std::size_t>(idx)] = lt::default_priority;
            }
        }

        for (const auto& override_entry : rules.overrides) {
            if (override_entry.index < priorities.size()) {
                priorities[override_entry.index] = to_priority(override_entry.priority);
            }
        }

        handle.prioritize_files(priorities);
    }

    bool is_fluff(const std::string& path) const {
        static const std::vector<std::regex> fluff = [] {
            std::vector<std::regex> compiled;
            compiled.reserve(kSkipFluffPatterns.size());
            for (const char* pattern : kSkipFluffPatterns) {
                compiled.emplace_back(glob_to_regex(pattern), std::regex::icase);
            }
            return compiled;
        }();

        return matches_any(fluff, path);
    }

    std::unique_ptr<lt::session> session_;
    std::string default_download_root_;
    std::string resume_dir_;
    bool sequential_default_{false};
    std::vector<std::string> default_trackers_;
    std::vector<std::string> extra_trackers_;
    bool replace_default_trackers_{false};
    bool announce_to_all_{false};
    std::unordered_map<std::string, lt::torrent_handle> handles_;
    std::unordered_map<std::string, TorrentSnapshot> snapshots_;
    std::unordered_map<std::string, std::vector<char>> pending_resume_;
    std::unordered_map<std::string, SelectionEntry> selection_rules_;
};

Session::Session(const SessionOptions& options)
    : impl_(std::make_unique<Impl>(options)) {}

Session::~Session() = default;

::rust::String Session::apply_engine_profile(const EngineOptions& options) {
    return impl_->apply_engine_profile(options);
}

::rust::String Session::add_torrent(const AddTorrentRequest& request) {
    return impl_->add_torrent(request);
}

::rust::String Session::remove_torrent(::rust::Str id, bool with_data) {
    return impl_->remove_torrent(to_std_string(id), with_data);
}

::rust::String Session::pause_torrent(::rust::Str id) {
    return impl_->pause_torrent(to_std_string(id));
}

::rust::String Session::resume_torrent(::rust::Str id) {
    return impl_->resume_torrent(to_std_string(id));
}

::rust::String Session::set_sequential(::rust::Str id, bool sequential) {
    return impl_->set_sequential(to_std_string(id), sequential);
}

::rust::String Session::load_fastresume(::rust::Str id, rust::Slice<const std::uint8_t> data) {
    return impl_->load_fastresume(to_std_string(id), data);
}

::rust::String Session::update_limits(const LimitRequest& request) {
    return impl_->update_limits(request);
}

::rust::String Session::update_selection(const SelectionRules& request) {
    return impl_->update_selection(request);
}

::rust::String Session::reannounce(::rust::Str id) {
    return impl_->reannounce(to_std_string(id));
}

::rust::String Session::recheck(::rust::Str id) {
    return impl_->recheck(to_std_string(id));
}

rust::Vec<NativeEvent> Session::poll_events() {
    return impl_->poll_events();
}

std::unique_ptr<Session> new_session(const SessionOptions& options) {
    return std::make_unique<Session>(options);
}

}  // namespace revaer
