#include "revaer/session.hpp"

#include "revaer-torrent-libt/src/ffi/bridge.rs.h"

#include <algorithm>
#include <array>
#include <cctype>
#include <chrono>
#include <cstdint>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <cstring>
#include <sstream>
#include <memory>
#include <optional>
#include <iomanip>
#include <regex>
#include <string>
#include <unordered_set>
#include <set>
#include <utility>
#include <limits>
#include <vector>
#include <iterator>

#include <libtorrent/add_torrent_params.hpp>
#include <libtorrent/alert.hpp>
#include <libtorrent/alert_types.hpp>
#include <libtorrent/bdecode.hpp>
#include <libtorrent/bencode.hpp>
#include <libtorrent/create_torrent.hpp>
#include <libtorrent/download_priority.hpp>
#include <libtorrent/error_code.hpp>
#include <libtorrent/address.hpp>
#include <libtorrent/hex.hpp>
#include <libtorrent/ip_filter.hpp>
#include <libtorrent/peer_class.hpp>
#include <libtorrent/peer_class_type_filter.hpp>
#include <libtorrent/socket_type.hpp>
#include <libtorrent/file_storage.hpp>
#include <libtorrent/info_hash.hpp>
#include <libtorrent/magnet_uri.hpp>
#include <libtorrent/peer_info.hpp>
#include <libtorrent/read_resume_data.hpp>
#include <libtorrent/session.hpp>
#include <libtorrent/session_params.hpp>
#include <libtorrent/settings_pack.hpp>
#include <libtorrent/session_types.hpp>
#include <libtorrent/storage_defs.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_info.hpp>
#include <libtorrent/torrent_flags.hpp>
#include <libtorrent/torrent_status.hpp>
#include <libtorrent/write_resume_data.hpp>
#include <openssl/evp.h>

namespace revaer {

namespace {

constexpr std::array<const char*, 5> kSkipFluffPatterns = {
    "**/sample/**",
    "**/samples/**",
    "**/extras/**",
    "**/proof/**",
    "**/screens/**",
};
constexpr std::size_t kMaxCreatePathLength = 4096;

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

std::vector<int> pick_sample_pieces(int total_pieces, int sample_count) {
    std::vector<int> pieces;
    pieces.reserve(sample_count);
    const int step = std::max(1, total_pieces / sample_count);
    std::unordered_set<int> seen;

    for (int piece = 0;
         static_cast<int>(pieces.size()) < sample_count && piece < total_pieces;
         piece += step) {
        if (seen.insert(piece).second) {
            pieces.push_back(piece);
        }
    }

    if (!pieces.empty() && pieces.back() != total_pieces - 1
        && static_cast<int>(pieces.size()) < sample_count) {
        if (seen.insert(total_pieces - 1).second) {
            pieces.push_back(total_pieces - 1);
        }
    }

    for (int candidate = 0;
         static_cast<int>(pieces.size()) < sample_count && candidate < total_pieces;
         ++candidate) {
        if (seen.insert(candidate).second) {
            pieces.push_back(candidate);
        }
    }

    return pieces;
}

lt::storage_mode_t to_storage_mode(int mode) {
    if (mode == 1) {
        return lt::storage_mode_allocate;
    }
    return lt::storage_mode_sparse;
}

struct MetainfoOverrides {
    bool has_comment{false};
    std::string comment;
    bool has_source{false};
    std::string source;
    bool has_private{false};
    bool private_flag{false};
};

struct MetainfoDetails {
    std::string comment;
    std::string source;
    bool has_private{false};
    bool private_flag{false};
};

MetainfoOverrides overrides_from_request(const AddTorrentRequest& request) {
    MetainfoOverrides overrides{};
    overrides.has_comment = request.has_comment;
    if (request.has_comment) {
        overrides.comment = to_std_string(request.comment);
    }
    overrides.has_source = request.has_source;
    if (request.has_source) {
        overrides.source = to_std_string(request.source);
    }
    overrides.has_private = request.has_private;
    if (request.has_private) {
        overrides.private_flag = request.private_flag;
    }
    return overrides;
}

MetainfoDetails extract_metainfo_details(const lt::torrent_info& info) {
    MetainfoDetails details{};
    details.comment = info.comment();
    {
        auto section = info.info_section();
        if (!section.empty()) {
            lt::error_code decode_ec;
            auto node = lt::bdecode(section, decode_ec);
            if (!decode_ec && node.type() == lt::bdecode_node::dict_t) {
                const auto source = node.dict_find_string_value("source");
                if (!source.empty()) {
                    details.source = std::string(source);
                }
            }
        }
    }
    details.private_flag = info.priv();
    details.has_private = true;
    return details;
}

bool apply_metainfo_overrides(lt::entry& metainfo,
                              const MetainfoOverrides& overrides,
                              std::string& error) {
    if (metainfo.type() != lt::entry::dictionary_t) {
        error = "metainfo root must be a dictionary";
        return false;
    }
    auto& root = metainfo.dict();
    auto info_it = root.find("info");
    if (info_it == root.end() || info_it->second.type() != lt::entry::dictionary_t) {
        error = "metainfo is missing an info dictionary";
        return false;
    }
    auto& info = info_it->second.dict();

    if (overrides.has_private) {
        if (overrides.private_flag) {
            info["private"] = 1;
        } else {
            info.erase("private");
        }
    }
    if (overrides.has_comment) {
        if (!overrides.comment.empty()) {
            root["comment"] = overrides.comment;
        } else {
            root.erase("comment");
        }
    }
    if (overrides.has_source) {
        if (!overrides.source.empty()) {
            info["source"] = overrides.source;
        } else {
            info.erase("source");
        }
    }

    return true;
}

std::optional<std::string> hash_sample(
    const lt::torrent_info& info,
    const std::string& save_path,
    std::uint8_t sample_pct) {
    if (sample_pct == 0) {
        return std::nullopt;
    }

    const int total_pieces = info.num_pieces();
    if (total_pieces <= 0) {
        return std::nullopt;
    }

    const auto sample_count = std::max(
        1,
        static_cast<int>(std::ceil(
            static_cast<double>(total_pieces) * static_cast<double>(sample_pct) / 100.0)));
    const auto pieces = pick_sample_pieces(total_pieces, sample_count);
    const auto& files = info.files();
    const std::filesystem::path root(save_path);

    for (int piece : pieces) {
        const int piece_size = info.piece_size(piece);
        std::unique_ptr<EVP_MD_CTX, decltype(&EVP_MD_CTX_free)> sha_ctx(
            EVP_MD_CTX_new(),
            &EVP_MD_CTX_free);
        if (!sha_ctx) {
            return std::string("seed-mode sample failed: unable to allocate sha1 ctx");
        }
        if (EVP_DigestInit_ex(sha_ctx.get(), EVP_sha1(), nullptr) != 1) {
            return std::string("seed-mode sample failed: unable to init sha1 digest");
        }
        const auto slices = files.map_block(piece, 0, piece_size);
        for (const auto& slice : slices) {
            const auto path = root / files.file_path(slice.file_index);
            std::ifstream file(path, std::ios::binary);
            if (!file) {
                return std::string("seed-mode sample failed: missing file ")
                    + path.string();
            }
            file.seekg(static_cast<std::streamoff>(slice.offset), std::ios::beg);
            std::vector<char> buffer;
            buffer.resize(static_cast<std::size_t>(slice.size));
            file.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
            if (file.gcount() != static_cast<std::streamsize>(buffer.size())) {
                return std::string("seed-mode sample failed: truncated file ")
                    + path.string();
            }
            if (EVP_DigestUpdate(
                    sha_ctx.get(),
                    reinterpret_cast<const unsigned char*>(buffer.data()),
                    buffer.size()) != 1) {
                return std::string("seed-mode sample failed: digest update error for file ")
                    + path.string();
            }
        }

        std::array<unsigned char, lt::sha1_hash::size()> digest{};
        unsigned int digest_len = 0;
        if (EVP_DigestFinal_ex(sha_ctx.get(), digest.data(), &digest_len) != 1) {
            return std::string("seed-mode sample failed: unable to finalize digest");
        }
        if (digest_len != lt::sha1_hash::size()) {
            return std::string("seed-mode sample failed: digest length mismatch");
        }

        const auto expected = info.hash_for_piece(piece);
        if (std::memcmp(expected.data(), digest.data(), lt::sha1_hash::size()) != 0) {
            return std::string("seed-mode sample failed: hash mismatch for piece ")
                + std::to_string(piece);
        }
    }

    return std::nullopt;
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
    bool resume_requested{false};
    std::string last_name;
    std::string last_download_dir;
};

bool set_bool_setting(lt::settings_pack& pack, const char* name, bool value) {
    const int index = lt::setting_by_name(name);
    if (index < 0) {
        return false;
    }
    pack.set_bool(index, value);
    return true;
}

bool get_bool_setting(const lt::settings_pack& pack, const char* name, bool fallback) {
    const int index = lt::setting_by_name(name);
    if (index < 0) {
        return fallback;
    }
    return pack.get_bool(index);
}

int get_int_setting(const lt::settings_pack& pack, const char* name, int fallback) {
    const int index = lt::setting_by_name(name);
    if (index < 0) {
        return fallback;
    }
    return pack.get_int(index);
}

bool set_int_setting(lt::settings_pack& pack, const char* name, int value) {
    const int index = lt::setting_by_name(name);
    if (index < 0) {
        return false;
    }
    pack.set_int(index, value);
    return true;
}

bool set_str_setting(lt::settings_pack& pack, const char* name, const std::string& value) {
    const int index = lt::setting_by_name(name);
    if (index < 0) {
        return false;
    }
    pack.set_str(index, value);
    return true;
}

void set_strict_super_seeding(lt::settings_pack& pack, bool value) {
    if (set_bool_setting(pack, "strict_super_seeding", value)) {
        return;
    }
    set_bool_setting(pack, "deprecated_strict_super_seeding", value);
}
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
        set_bool_setting(pack, "force_proxy", false);
        pack.set_bool(lt::settings_pack::prefer_rc4, false);
        pack.set_bool(lt::settings_pack::allow_multiple_connections_per_ip, false);
        pack.set_int(lt::settings_pack::alert_mask,
                     lt::alert_category::status | lt::alert_category::error |
                         lt::alert_category::storage | lt::alert_category::file_progress |
                         lt::alert_category::tracker);

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
            set_bool_setting(pack, "force_proxy", options.network.force_proxy);
            pack.set_bool(lt::settings_pack::prefer_rc4, options.network.prefer_rc4);
            pack.set_bool(lt::settings_pack::allow_multiple_connections_per_ip,
                          options.network.allow_multiple_connections_per_ip);
            pack.set_bool(lt::settings_pack::auto_manage_prefer_seeds,
                          options.behavior.auto_manage_prefer_seeds);
            pack.set_bool(lt::settings_pack::dont_count_slow_torrents,
                          options.behavior.dont_count_slow_torrents);

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

            if (options.network.has_outgoing_port_range &&
                options.network.outgoing_port_min > 0 &&
                options.network.outgoing_port_max >= options.network.outgoing_port_min) {
                const int min_port = options.network.outgoing_port_min;
                const int max_port = options.network.outgoing_port_max;
                const int range = std::max(0, max_port - min_port + 1);
                pack.set_int(lt::settings_pack::outgoing_port, min_port);
                pack.set_int(lt::settings_pack::num_outgoing_ports, range);
            } else {
                pack.set_int(lt::settings_pack::outgoing_port, 0);
                pack.set_int(lt::settings_pack::num_outgoing_ports, 0);
            }

            if (options.network.has_peer_dscp) {
                pack.set_int(lt::settings_pack::peer_dscp, options.network.peer_dscp);
            } else {
                pack.set_int(lt::settings_pack::peer_dscp, 0);
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
            if (options.limits.connections_limit >= 0) {
                pack.set_int(lt::settings_pack::connections_limit,
                             options.limits.connections_limit);
            }
            default_max_connections_per_torrent_ = options.limits.connections_limit_per_torrent;
            if (options.limits.unchoke_slots >= 0) {
                pack.set_int(lt::settings_pack::unchoke_slots_limit,
                             options.limits.unchoke_slots);
            }
            if (options.limits.half_open_limit >= 0) {
                set_int_setting(pack, "half_open_limit", options.limits.half_open_limit);
            }

            pack.set_int(lt::settings_pack::choking_algorithm,
                         options.limits.choking_algorithm);
            pack.set_int(lt::settings_pack::seed_choking_algorithm,
                         options.limits.seed_choking_algorithm);
            set_strict_super_seeding(pack, options.limits.strict_super_seeding);

            if (options.limits.has_optimistic_unchoke_slots) {
                pack.set_int(lt::settings_pack::num_optimistic_unchoke_slots,
                             options.limits.optimistic_unchoke_slots);
            }

            if (options.limits.has_max_queued_disk_bytes) {
                pack.set_int(lt::settings_pack::max_queued_disk_bytes,
                             options.limits.max_queued_disk_bytes);
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
            default_storage_mode_ = to_storage_mode(options.storage.storage_mode);
            set_bool_setting(pack, "use_partfile", options.storage.use_partfile);
            if (options.storage.has_disk_read_mode) {
                set_int_setting(pack, "disk_io_read_mode", options.storage.disk_read_mode);
            }
            if (options.storage.has_disk_write_mode) {
                set_int_setting(pack, "disk_io_write_mode", options.storage.disk_write_mode);
            }
            set_bool_setting(
                pack, "disable_hash_checks", !options.storage.verify_piece_hashes);
            if (options.storage.has_cache_size) {
                set_int_setting(pack, "cache_size", options.storage.cache_size);
            }
            if (options.storage.has_cache_expiry) {
                set_int_setting(pack, "cache_expiry", options.storage.cache_expiry);
            }
            set_bool_setting(pack, "coalesce_reads", options.storage.coalesce_reads);
            set_bool_setting(pack, "coalesce_writes", options.storage.coalesce_writes);
            set_bool_setting(pack, "use_disk_cache_pool", options.storage.use_disk_cache_pool);

            sequential_default_ = options.behavior.sequential_default;
            auto_managed_default_ = options.behavior.auto_managed;
            pex_enabled_ = options.network.enable_pex;
            super_seeding_default_ = options.behavior.super_seeding;

            pack.set_int(
                lt::settings_pack::download_rate_limit,
                options.limits.download_rate_limit >= 0
                    ? static_cast<int>(options.limits.download_rate_limit)
                    : -1);
            pack.set_int(lt::settings_pack::upload_rate_limit,
                         options.limits.upload_rate_limit >= 0
                             ? static_cast<int>(options.limits.upload_rate_limit)
                             : -1);
            if (options.limits.has_seed_ratio_limit) {
                // libtorrent expects share ratio limit scaled by 1000.
                const double scaled = std::clamp(
                    options.limits.seed_ratio_limit * 1000.0,
                    0.0,
                    static_cast<double>(std::numeric_limits<int>::max()));
                pack.set_int(lt::settings_pack::share_ratio_limit,
                             static_cast<int>(scaled));
            } else {
                pack.set_int(lt::settings_pack::share_ratio_limit, -1);
            }
            if (options.limits.has_seed_time_limit) {
                const auto clamped = static_cast<int>(std::clamp(
                    options.limits.seed_time_limit,
                    static_cast<std::int64_t>(0),
                    static_cast<std::int64_t>(std::numeric_limits<int>::max())));
                pack.set_int(lt::settings_pack::seed_time_limit, clamped);
            } else {
                pack.set_int(lt::settings_pack::seed_time_limit, -1);
            }
            if (options.limits.has_stats_interval) {
                pack.set_int(lt::settings_pack::tick_interval,
                             std::max(1, options.limits.stats_interval_ms));
            }

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
            if (options.tracker.has_ssl_cert) {
                set_str_setting(pack, "ssl_cert", to_std_string(options.tracker.ssl_cert));
            }
            if (options.tracker.has_ssl_private_key) {
                set_str_setting(
                    pack, "ssl_private_key", to_std_string(options.tracker.ssl_private_key));
            }
            if (options.tracker.has_ssl_ca_cert) {
                set_str_setting(
                    pack, "ssl_ca_cert", to_std_string(options.tracker.ssl_ca_cert));
            }
            if (options.tracker.has_ssl_tracker_verify) {
                set_bool_setting(
                    pack, "ssl_tracker_verify", options.tracker.ssl_tracker_verify);
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

            tracker_username_.clear();
            tracker_password_.clear();
            tracker_cookie_.clear();
            has_tracker_username_ = options.tracker.auth.has_username;
            has_tracker_password_ = options.tracker.auth.has_password;
            has_tracker_cookie_ = options.tracker.auth.has_cookie;
            if (has_tracker_username_) {
                tracker_username_ = to_std_string(options.tracker.auth.username);
            }
            if (has_tracker_password_) {
                tracker_password_ = to_std_string(options.tracker.auth.password);
            }
            if (has_tracker_cookie_) {
                tracker_cookie_ = to_std_string(options.tracker.auth.cookie);
            }

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

            configure_peer_classes(options);

            session_->apply_settings(pack);
        } catch (const std::exception& ex) {
            return ::rust::String(ex.what());
        }
        return ::rust::String();
    }

    void configure_peer_classes(const EngineOptions& options) {
        for (const auto cid : custom_peer_classes_) {
            session_->delete_peer_class(cid);
        }
        custom_peer_classes_.clear();
        peer_class_map_.fill(lt::peer_class_t{0});
        configured_peer_classes_.clear();
        default_peer_classes_.clear();

        for (const auto& cfg : options.peer_classes) {
            const std::string label = to_std_string(cfg.label);
            lt::peer_class_t cid = session_->create_peer_class(label.c_str());
            lt::peer_class_info info{};
            info.ignore_unchoke_slots = cfg.ignore_unchoke_slots;
            info.connection_limit_factor = static_cast<int>(cfg.connection_limit_factor);
            info.label = label;
            info.upload_limit = 0;
            info.download_limit = 0;
            info.upload_priority = cfg.upload_priority;
            info.download_priority = cfg.download_priority;
            session_->set_peer_class(cid, info);

            const std::size_t idx = static_cast<std::size_t>(cfg.id);
            if (idx < peer_class_map_.size()) {
                peer_class_map_[idx] = cid;
            }
            custom_peer_classes_.push_back(cid);
            configured_peer_classes_.push_back(cfg.id);
        }

        lt::peer_class_type_filter filter;
        constexpr std::array<lt::peer_class_type_filter::socket_type_t, 5> socket_types = {
            lt::peer_class_type_filter::tcp_socket,
            lt::peer_class_type_filter::utp_socket,
            lt::peer_class_type_filter::ssl_tcp_socket,
            lt::peer_class_type_filter::ssl_utp_socket,
            lt::peer_class_type_filter::i2p_socket};
        for (const auto cid : options.default_peer_classes) {
            const std::size_t idx = static_cast<std::size_t>(cid);
            if (idx >= peer_class_map_.size()) {
                continue;
            }
            const lt::peer_class_t mapped = peer_class_map_[idx];
            if (mapped == lt::peer_class_t{0}) {
                continue;
            }
            for (const auto st : socket_types) {
                filter.add(st, mapped);
            }
            default_peer_classes_.push_back(static_cast<std::uint8_t>(idx));
        }
        session_->set_peer_class_type_filter(filter);
    }

    CreateTorrentResult create_torrent(const CreateTorrentRequest& request) {
        CreateTorrentResult result{};
        result.metainfo = rust::Vec<std::uint8_t>();
        result.files = rust::Vec<CreateTorrentFile>();
        result.warnings = rust::Vec<rust::String>();
        result.trackers = rust::Vec<rust::String>();
        result.web_seeds = rust::Vec<rust::String>();
        result.magnet_uri = ::rust::String();
        result.info_hash = ::rust::String();
        result.error = ::rust::String();
        result.private_flag = request.private_flag;
        result.comment = request.has_comment ? to_std_string(request.comment) : std::string();
        result.source = request.has_source ? to_std_string(request.source) : std::string();

        std::vector<std::string> warnings;
        try {
            const std::string root = to_std_string(request.root_path);
            if (root.empty()) {
                result.error = "root_path is required";
                return result;
            }

            std::filesystem::path root_path(root);
            std::error_code fs_ec;
            const auto status = std::filesystem::status(root_path, fs_ec);
            if (fs_ec || (!std::filesystem::is_regular_file(status)
                          && !std::filesystem::is_directory(status))) {
                result.error = "root_path must point to a file or directory";
                return result;
            }

            const bool is_file = std::filesystem::is_regular_file(status);
            const auto append_warning = [&warnings](const std::string& message) {
                warnings.push_back(message);
            };

            auto compile_patterns = [](const rust::Vec<rust::String>& patterns) {
                std::vector<std::regex> compiled;
                compiled.reserve(patterns.size());
                for (const auto& pattern : patterns) {
                    compiled.emplace_back(glob_to_regex(to_std_string(pattern)), std::regex::icase);
                }
                return compiled;
            };

            const auto include_patterns = compile_patterns(request.include);
            const auto exclude_patterns = compile_patterns(request.exclude);

            struct FileEntry {
                std::string path;
                std::uint64_t size;
            };

            std::vector<FileEntry> files;
            std::unordered_set<std::string> seen;
            std::size_t skipped = 0;
            std::vector<std::string> skipped_samples;

            auto record_skip = [&skipped, &skipped_samples](const std::string& path) {
                ++skipped;
                if (skipped_samples.size() < 5) {
                    skipped_samples.push_back(path);
                }
            };

            auto should_include = [&](const std::string& rel_path) -> bool {
                if (rel_path.size() > kMaxCreatePathLength) {
                    record_skip(rel_path);
                    return false;
                }
                if (request.skip_fluff && is_fluff(rel_path)) {
                    record_skip(rel_path);
                    return false;
                }
                if (!exclude_patterns.empty() && matches_any(exclude_patterns, rel_path)) {
                    record_skip(rel_path);
                    return false;
                }
                if (!include_patterns.empty() && !matches_any(include_patterns, rel_path)) {
                    record_skip(rel_path);
                    return false;
                }
                return true;
            };

            auto add_file = [&](const std::filesystem::path& full_path,
                                const std::filesystem::path& relative_path) {
                const std::string rel = relative_path.generic_string();
                if (!should_include(rel)) {
                    return;
                }
                if (!seen.insert(rel).second) {
                    throw std::runtime_error("duplicate file path: " + rel);
                }
                std::error_code size_ec;
                const auto size = std::filesystem::file_size(full_path, size_ec);
                if (size_ec) {
                    throw std::runtime_error("failed to read file size for " + rel);
                }
                files.push_back(FileEntry{rel, static_cast<std::uint64_t>(size)});
            };

            if (is_file) {
                add_file(root_path, root_path.filename());
            } else {
                for (std::filesystem::recursive_directory_iterator it(root_path, fs_ec), end;
                     it != end;
                     it.increment(fs_ec)) {
                    if (fs_ec) {
                        throw std::runtime_error("failed to traverse root_path");
                    }
                    if (!it->is_regular_file()) {
                        continue;
                    }
                    const auto rel_path = it->path().lexically_relative(root_path);
                    add_file(it->path(), rel_path);
                }
            }

            if (files.empty()) {
                result.error = "no files matched the authoring rules";
                return result;
            }

            std::sort(files.begin(), files.end(), [](const auto& left, const auto& right) {
                return left.path < right.path;
            });

            if (skipped > 0) {
                std::ostringstream message;
                message << "skipped " << skipped << " files due to filters";
                if (!skipped_samples.empty()) {
                    message << " (e.g. ";
                    for (std::size_t idx = 0; idx < skipped_samples.size(); ++idx) {
                        if (idx > 0) {
                            message << ", ";
                        }
                        message << skipped_samples[idx];
                    }
                    message << ")";
                }
                append_warning(message.str());
            }

            lt::file_storage storage;
            const std::string name = root_path.filename().string();
            if (!name.empty()) {
                storage.set_name(name);
            }

            std::uint64_t total_size = 0;
            for (const auto& entry : files) {
                storage.add_file(entry.path, entry.size);
                total_size += entry.size;
            }

            const auto normalize_piece = [&](std::uint32_t value) -> std::uint32_t {
                constexpr std::uint32_t kMinPiece = 16 * 1024;
                constexpr std::uint32_t kMaxPiece = 16 * 1024 * 1024;
                if (value < kMinPiece) {
                    return kMinPiece;
                }
                if (value > kMaxPiece) {
                    return kMaxPiece;
                }
                if ((value & (value - 1)) == 0) {
                    return value;
                }
                std::uint32_t next = kMinPiece;
                while (next < value && next < kMaxPiece) {
                    next <<= 1;
                }
                return std::min(next, kMaxPiece);
            };

            std::uint32_t piece_length = 0;
            if (request.has_piece_length) {
                piece_length = normalize_piece(request.piece_length);
                if (piece_length != request.piece_length) {
                    append_warning("piece_length was adjusted to a supported value");
                }
            }

            std::vector<std::string> trackers;
            {
                std::unordered_set<std::string> seen_tracker;
                for (const auto& tracker : request.trackers) {
                    const auto value = to_std_string(tracker);
                    if (value.empty()) {
                        continue;
                    }
                    if (seen_tracker.insert(value).second) {
                        trackers.push_back(value);
                    }
                }
            }

            if (request.private_flag && trackers.empty()) {
                result.error = "private torrents require at least one tracker";
                return result;
            }

            std::vector<std::string> web_seeds;
            {
                std::unordered_set<std::string> seen_seed;
                for (const auto& seed : request.web_seeds) {
                    const auto value = to_std_string(seed);
                    if (value.empty()) {
                        continue;
                    }
                    if (seen_seed.insert(value).second) {
                        web_seeds.push_back(value);
                    }
                }
            }

            const int piece_length_value =
                request.has_piece_length ? static_cast<int>(piece_length) : 0;
            lt::create_torrent builder(storage, piece_length_value);
            if (request.private_flag) {
                builder.set_priv(true);
            }
            if (request.has_comment && !result.comment.empty()) {
                builder.set_comment(result.comment.c_str());
            }
            for (const auto& tracker : trackers) {
                builder.add_tracker(tracker);
            }
            for (const auto& seed : web_seeds) {
                builder.add_url_seed(seed);
            }

            const auto hash_root =
                is_file ? root_path.parent_path().string() : root_path.string();
            lt::error_code hash_ec;
            lt::set_piece_hashes(builder, hash_root, hash_ec);
            if (hash_ec) {
                result.error = "hashing failed: " + hash_ec.message();
                return result;
            }

            lt::entry metainfo_entry = builder.generate();
            if (request.has_source && !result.source.empty()) {
                metainfo_entry["info"]["source"] = result.source;
            }

            std::vector<char> buffer;
            lt::bencode(std::back_inserter(buffer), metainfo_entry);

            lt::error_code info_ec;
            const int buffer_size = static_cast<int>(buffer.size());
            lt::torrent_info info(buffer.data(), buffer_size, info_ec);
            if (info_ec) {
                result.error = "metainfo parse failed: " + info_ec.message();
                return result;
            }

            result.metainfo.reserve(buffer.size());
            for (char byte : buffer) {
                result.metainfo.push_back(static_cast<std::uint8_t>(byte));
            }
            result.magnet_uri = lt::make_magnet_uri(info);
            result.info_hash =
                lt::aux::to_hex(info.info_hashes().get_best().to_string());
            const int effective_piece_length = builder.piece_length();
            result.piece_length =
                effective_piece_length > 0
                    ? static_cast<std::uint32_t>(effective_piece_length)
                    : piece_length;
            result.total_size = total_size;

            result.files.reserve(files.size());
            for (const auto& entry : files) {
                CreateTorrentFile file{};
                file.path = entry.path;
                file.size_bytes = entry.size;
                result.files.push_back(std::move(file));
            }
            result.trackers.reserve(trackers.size());
            for (const auto& tracker : trackers) {
                result.trackers.push_back(tracker);
            }
            result.web_seeds.reserve(web_seeds.size());
            for (const auto& seed : web_seeds) {
                result.web_seeds.push_back(seed);
            }
            result.warnings.reserve(warnings.size());
            for (const auto& warning : warnings) {
                result.warnings.push_back(warning);
            }
        } catch (const std::exception& ex) {
            result.error = ex.what();
        }
        return result;
    }

    ::rust::String add_torrent(const AddTorrentRequest& request) {
        try {
            lt::add_torrent_params params;
            const auto overrides = overrides_from_request(request);
            std::vector<char> metainfo_buffer;
            const auto request_id = to_std_string(request.id);
            const auto download_dir = to_std_string(request.download_dir);
            auto resume_it = pending_resume_.find(request_id);
            if (resume_it != pending_resume_.end()) {
                lt::error_code resume_ec;
                auto resume_params = lt::read_resume_data(resume_it->second, resume_ec);
                pending_resume_.erase(resume_it);
                if (resume_ec) {
                    return ::rust::String(
                        "resume data parse failed: " + resume_ec.message());
                }
                params = std::move(resume_params);
                if (params.save_path.empty()) {
                    params.save_path =
                        request.has_download_dir ? download_dir : default_download_root_;
                } else if (request.has_download_dir) {
                    params.save_path = download_dir;
                }
            } else {
                params.save_path = request.has_download_dir ? download_dir : default_download_root_;
                if (params.save_path.empty()) {
                    return "download directory not configured";
                }

                if (request.source_kind == SourceKind::Magnet) {
                    auto parsed = lt::parse_magnet_uri(to_std_string(request.magnet_uri));
                    parsed.save_path =
                        request.has_download_dir ? download_dir : default_download_root_;
                    params = std::move(parsed);
                } else {
                    if (request.metainfo.empty()) {
                        return "metainfo payload empty";
                    }
                    metainfo_buffer.resize(request.metainfo.size());
                    std::transform(
                        request.metainfo.begin(),
                        request.metainfo.end(),
                        metainfo_buffer.begin(),
                        [](std::uint8_t byte) { return static_cast<char>(byte); });
                    if (overrides.has_comment || overrides.has_source || overrides.has_private) {
                        lt::error_code decode_ec;
                        lt::bdecode_node decoded;
                        const int decode_result = lt::bdecode(
                            metainfo_buffer.data(),
                            metainfo_buffer.data() + metainfo_buffer.size(),
                            decoded,
                            decode_ec);
                        if (decode_result != 0 || decode_ec) {
                            return ::rust::String(
                                "metainfo decode failed: " + decode_ec.message());
                        }
                        lt::entry metainfo_entry(decoded);
                        std::string override_error;
                        if (!apply_metainfo_overrides(metainfo_entry, overrides, override_error)) {
                            return ::rust::String(override_error);
                        }
                        std::vector<char> updated;
                        lt::bencode(std::back_inserter(updated), metainfo_entry);
                        metainfo_buffer = std::move(updated);
                    }

                    lt::span<const char> buffer(
                        metainfo_buffer.data(),
                        static_cast<long>(metainfo_buffer.size()));
                    lt::error_code parse_ec;
                    params.ti = std::make_shared<lt::torrent_info>(
                        buffer,
                        parse_ec,
                        lt::from_span);
                    if (parse_ec) {
                        return ::rust::String(
                            "metainfo parse failed (bytes="
                            + std::to_string(metainfo_buffer.size())
                            + "): " + parse_ec.message());
                    }
                }
            }

            const bool seed_mode_requested = request.has_seed_mode && request.seed_mode;
            const bool hash_sample_requested =
                request.has_hash_check_sample && request.hash_check_sample_pct > 0;

            if (seed_mode_requested && !params.ti) {
                return "seed_mode requires metainfo payload";
            }

            if (hash_sample_requested) {
                if (!params.ti) {
                    return "hash sample requires metainfo payload";
                }
                const auto sample_result =
                    hash_sample(*params.ti, params.save_path, request.hash_check_sample_pct);
                if (sample_result.has_value()) {
                    return ::rust::String(*sample_result);
                }
            }

            const bool auto_managed = request.has_auto_managed
                ? request.auto_managed
                : (request.has_queue_position ? false : auto_managed_default_);
            const bool pex_enabled =
                request.has_pex_enabled ? request.pex_enabled : pex_enabled_;
            const bool super_seeding = request.has_super_seeding
                ? request.super_seeding
                : super_seeding_default_;
            if (auto_managed) {
                params.flags |= lt::torrent_flags::auto_managed;
            } else {
                params.flags &= ~lt::torrent_flags::auto_managed;
            }
            if (pex_enabled) {
                params.flags &= ~lt::torrent_flags::disable_pex;
            } else {
                params.flags |= lt::torrent_flags::disable_pex;
            }
            if (seed_mode_requested) {
                params.flags |= lt::torrent_flags::seed_mode;
            } else {
                params.flags &= ~lt::torrent_flags::seed_mode;
            }
            if (super_seeding) {
                params.flags |= lt::torrent_flags::super_seeding;
            } else {
                params.flags &= ~lt::torrent_flags::super_seeding;
            }
            if (request.has_start_paused && request.start_paused) {
                params.flags |= lt::torrent_flags::paused;
            }
            if (request.has_max_connections && request.max_connections > 0) {
                params.max_connections = request.max_connections;
            } else if (default_max_connections_per_torrent_ > 0) {
                params.max_connections = default_max_connections_per_torrent_;
            }

            const AuthView auth = resolve_auth_view(request.tracker_auth);

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
                params.trackers = apply_tracker_auth(trackers, auth);
            }

            if (overrides.has_private && overrides.private_flag) {
                bool has_tracker = !params.trackers.empty();
                if (!has_tracker && params.ti) {
                    has_tracker = !params.ti->trackers().empty();
                }
                if (!has_tracker) {
                    return ::rust::String("private torrents require at least one tracker");
                }
            }

            if (request.tracker_auth.has_cookie) {
                params.trackerid = to_std_string(request.tracker_auth.cookie);
            } else if (has_tracker_cookie_) {
                params.trackerid = tracker_cookie_;
            }

            if (!request.web_seeds.empty()) {
                std::vector<std::string> seeds;
                seeds.reserve(request.web_seeds.size());
                for (const auto& seed : request.web_seeds) {
                    seeds.push_back(to_std_string(seed));
                }
                if (request.replace_web_seeds) {
                    params.url_seeds = std::move(seeds);
                } else if (!params.url_seeds.empty()) {
                    std::unordered_set<std::string> seen;
                    for (const auto& existing : params.url_seeds) {
                        seen.insert(existing);
                    }
                    for (const auto& seed : seeds) {
                        if (seen.insert(seed).second) {
                            params.url_seeds.push_back(seed);
                        }
                    }
                } else {
                    params.url_seeds = std::move(seeds);
                }
            }

            if (request.has_storage_mode) {
                params.storage_mode = to_storage_mode(request.storage_mode);
            } else {
                params.storage_mode = default_storage_mode_;
            }

            lt::torrent_handle handle = session_->add_torrent(params);
            handles_[request_id] = handle;
            snapshots_[request_id] = TorrentSnapshot{};

            if (request.has_queue_position && request.queue_position >= 0) {
                handle.queue_position_set(lt::queue_position_t{request.queue_position});
            }

            if (request.has_max_connections && request.max_connections > 0) {
                handle.set_max_connections(request.max_connections);
            }

            bool sequential = sequential_default_;
            if (request.has_sequential_override) {
                sequential = request.sequential;
            }
            if (sequential) {
                handle.set_flags(lt::torrent_flags::sequential_download);
            } else {
                handle.unset_flags(lt::torrent_flags::sequential_download);
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

    ::rust::String update_options(const UpdateOptionsRequest& request) {
        const auto key = to_std_string(request.id);
        if (request.has_private) {
            return ::rust::String("private flag updates are not supported");
        }
        if (request.has_source) {
            return ::rust::String("source updates are not supported");
        }
        return mutate_handle(key, [&](lt::torrent_handle& handle) {
            if (request.has_max_connections) {
                handle.set_max_connections(request.max_connections);
            }
            if (request.has_pex_enabled) {
                if (request.pex_enabled) {
                    handle.unset_flags(lt::torrent_flags::disable_pex);
                } else {
                    handle.set_flags(lt::torrent_flags::disable_pex);
                }
            }
            if (request.has_super_seeding) {
                if (request.super_seeding) {
                    handle.set_flags(lt::torrent_flags::super_seeding);
                } else {
                    handle.unset_flags(lt::torrent_flags::super_seeding);
                }
            }
            if (request.has_auto_managed) {
                if (request.auto_managed) {
                    handle.set_flags(lt::torrent_flags::auto_managed);
                } else {
                    handle.unset_flags(lt::torrent_flags::auto_managed);
                }
            }
            if (request.has_queue_position) {
                handle.queue_position_set(lt::queue_position_t{request.queue_position});
            }
        });
    }

    ::rust::String update_trackers(const UpdateTrackersRequest& request) {
        const auto key = to_std_string(request.id);
        const AuthView auth{
            .username = tracker_username_,
            .password = tracker_password_,
            .has_username = has_tracker_username_,
            .has_password = has_tracker_password_,
        };
        return mutate_handle(key, [&](lt::torrent_handle& handle) {
            std::vector<lt::announce_entry> trackers;
            if (!request.replace) {
                trackers = handle.trackers();
            }
            std::unordered_set<std::string> seen;
            for (const auto& entry : trackers) {
                seen.insert(entry.url);
            }
            for (const auto& tracker : request.trackers) {
                auto url = to_std_string(tracker);
                if (url.empty()) {
                    continue;
                }
                auto rewritten = inject_basic_auth(url, auth);
                if (seen.insert(rewritten).second) {
                    trackers.emplace_back(rewritten);
                }
            }
            if (!trackers.empty()) {
                handle.replace_trackers(trackers);
            }
        });
    }

    ::rust::String update_web_seeds(const UpdateWebSeedsRequest& request) {
        const auto key = to_std_string(request.id);
        return mutate_handle(key, [&](lt::torrent_handle& handle) {
            std::unordered_set<std::string> seeds;
            if (!request.replace) {
                for (const auto& seed : handle.url_seeds()) {
                    seeds.insert(seed);
                }
            }
            for (const auto& seed : request.web_seeds) {
                auto value = to_std_string(seed);
                if (!value.empty()) {
                    seeds.insert(std::move(value));
                }
            }
            if (request.replace) {
                for (const auto& existing : handle.url_seeds()) {
                    if (seeds.find(existing) == seeds.end()) {
                        handle.remove_url_seed(existing);
                    }
                }
            }
            for (const auto& seed : seeds) {
                handle.add_url_seed(seed);
            }
        });
    }

    ::rust::String move_torrent(const MoveTorrentRequest& request) {
        const auto key = to_std_string(request.id);
        const auto target = to_std_string(request.download_dir);
        return mutate_handle(key, [&](lt::torrent_handle& handle) {
            handle.move_storage(target, lt::move_flags_t::dont_replace);
        });
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

    ::rust::String set_piece_deadline(::rust::Str id, std::uint32_t piece, std::int32_t deadline_ms, bool has_deadline) {
        return mutate_handle(to_std_string(id), [piece, deadline_ms, has_deadline](lt::torrent_handle& handle) {
            lt::piece_index_t target{static_cast<int>(piece)};
            if (has_deadline) {
                handle.set_piece_deadline(target, deadline_ms);
            } else {
                handle.reset_piece_deadline(target);
            }
        });
    }

    rust::Vec<NativePeerInfo> list_peers(::rust::Str id) {
        rust::Vec<NativePeerInfo> peers_out;
        const auto key = to_std_string(id);
        auto it = handles_.find(key);
        if (it == handles_.end()) {
            return peers_out;
        }
        std::vector<lt::peer_info> peers;
        it->second.get_peer_info(peers);
        peers_out.reserve(peers.size());
        for (const auto& peer : peers) {
            NativePeerInfo info{};
            const auto address = peer.ip.address().to_string();
            const auto port = peer.ip.port();
            if (port > 0) {
                info.endpoint = address + ":" + std::to_string(port);
            } else {
                info.endpoint = address;
            }
            info.client = peer.client;
            info.progress = peer.progress;
            info.download_rate = static_cast<std::int64_t>(peer.down_speed);
            info.upload_rate = static_cast<std::int64_t>(peer.up_speed);
            info.interesting = (peer.flags & lt::peer_info::interesting) != 0;
            info.choked = (peer.flags & lt::peer_info::choked) != 0;
            info.remote_interested =
                (peer.flags & lt::peer_info::remote_interested) != 0;
            info.remote_choked = (peer.flags & lt::peer_info::remote_choked) != 0;
            peers_out.push_back(std::move(info));
        }
        return peers_out;
    }

    rust::Vec<NativeEvent> poll_events() {
        rust::Vec<NativeEvent> events;

        auto push_session_error = [&events](
                                      std::string component,
                                      std::string message,
                                      std::string id) {
            NativeEvent evt{};
            evt.id = std::move(id);
            evt.kind = NativeEventKind::SessionError;
            evt.state = NativeTorrentState::Failed;
            evt.component = std::move(component);
            evt.message = std::move(message);
            events.push_back(std::move(evt));
        };

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
            if (auto* tracker_err = lt::alert_cast<lt::tracker_error_alert>(alert)) {
                auto id = find_torrent_id(tracker_err->handle);
                if (!id.empty()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::TrackerUpdate;
                    evt.state = NativeTorrentState::Downloading;
                    evt.tracker_statuses = rust::Vec<NativeTrackerStatus>();
                    NativeTrackerStatus status{};
                    status.url = tracker_err->tracker_url();
                    status.status = "error";
                    status.message = tracker_err->error.message();
                    evt.tracker_statuses.push_back(std::move(status));
                    events.push_back(evt);
                }
            }
            if (auto* listen_err = lt::alert_cast<lt::listen_failed_alert>(alert)) {
                push_session_error("network", listen_err->error.message(), std::string());
            }
            if (auto* portmap_err = lt::alert_cast<lt::portmap_error_alert>(alert)) {
                push_session_error("portmap", portmap_err->error.message(), std::string());
            }
            if (auto* storage_err = lt::alert_cast<lt::file_error_alert>(alert)) {
                auto id = find_torrent_id(storage_err->handle);
                NativeEvent evt{};
                evt.id = id;
                evt.kind = NativeEventKind::Error;
                evt.state = NativeTorrentState::Failed;
                evt.message = storage_err->error.message();
                events.push_back(evt);
                push_session_error("storage", storage_err->error.message(), id);
            }
            if (auto* tracker_warn = lt::alert_cast<lt::tracker_warning_alert>(alert)) {
                auto id = find_torrent_id(tracker_warn->handle);
                if (!id.empty()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::TrackerUpdate;
                    evt.state = NativeTorrentState::Downloading;
                    evt.tracker_statuses = rust::Vec<NativeTrackerStatus>();
                    NativeTrackerStatus status{};
                    status.url = tracker_warn->tracker_url();
                    status.status = "warning";
                    status.message = tracker_warn->message();
                    evt.tracker_statuses.push_back(std::move(status));
                    events.push_back(evt);
                }
            }
            if (auto* tracker_err =
                    lt::alert_cast<lt::tracker_error_alert>(alert)) {
                auto id = find_torrent_id(tracker_err->handle);
                push_session_error("tracker", tracker_err->error.message(), id);
            }
            if (auto* peer_ban = lt::alert_cast<lt::peer_ban_alert>(alert)) {
                auto id = find_torrent_id(peer_ban->handle);
                push_session_error("peer", peer_ban->message(), id);
            }
            if (auto* peer_error = lt::alert_cast<lt::peer_error_alert>(alert)) {
                auto id = find_torrent_id(peer_error->handle);
                push_session_error("peer", peer_error->message(), id);
            }
            if (auto* peer_blocked =
                    lt::alert_cast<lt::peer_blocked_alert>(alert)) {
                auto id = find_torrent_id(peer_blocked->handle);
                push_session_error("peer", peer_blocked->message(), id);
            }
            if (auto* cert = lt::alert_cast<lt::torrent_need_cert_alert>(alert)) {
                auto id = find_torrent_id(cert->handle);
                push_session_error("ssl", cert->message(), id);
            }
            if (auto* moved = lt::alert_cast<lt::storage_moved_alert>(alert)) {
                auto id = find_torrent_id(moved->handle);
                auto snapshot = snapshots_.find(id);
                if (!id.empty() && snapshot != snapshots_.end()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::MetadataUpdated;
                    evt.state = snapshot->second.state;
                    evt.name = snapshot->second.last_name;
                    evt.download_dir = moved->storage_path();
                    if (auto info = moved->handle.torrent_file()) {
                        const auto details = extract_metainfo_details(*info);
                        evt.comment = details.comment;
                        evt.source = details.source;
                        evt.private_flag = details.private_flag;
                        evt.has_private = details.has_private;
                    }
                    events.push_back(evt);
                    snapshot->second.last_download_dir = moved->storage_path();
                }
            }
            if (auto* move_failed = lt::alert_cast<lt::storage_moved_failed_alert>(alert)) {
                auto id = find_torrent_id(move_failed->handle);
                auto snapshot = snapshots_.find(id);
                if (!id.empty() && snapshot != snapshots_.end()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::Error;
                    evt.state = snapshot->second.state;
                    evt.message = move_failed->error.message();
                    events.push_back(evt);
                }
            }
            if (auto* resume = lt::alert_cast<lt::save_resume_data_alert>(alert)) {
                auto id = find_torrent_id(resume->handle);
                auto snapshot = snapshots_.find(id);
                if (!id.empty() && snapshot != snapshots_.end()) {
                    auto buffer = lt::write_resume_data_buf(resume->params);
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::ResumeData;
                    evt.state = snapshot->second.state;
                    evt.resume_data = rust::Vec<std::uint8_t>();
                    evt.resume_data.reserve(buffer.size());
                    for (auto byte : buffer) {
                        evt.resume_data.push_back(static_cast<std::uint8_t>(byte));
                    }
                    events.push_back(evt);
                    snapshot->second.resume_requested = false;
                }
            }
            if (auto* resume_failed = lt::alert_cast<lt::save_resume_data_failed_alert>(alert)) {
                auto id = find_torrent_id(resume_failed->handle);
                auto snapshot = snapshots_.find(id);
                if (!id.empty() && snapshot != snapshots_.end()) {
                    NativeEvent evt{};
                    evt.id = id;
                    evt.kind = NativeEventKind::Error;
                    evt.state = snapshot->second.state;
                    evt.message = resume_failed->message();
                    events.push_back(evt);
                    snapshot->second.resume_requested = false;
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

                    const auto details = extract_metainfo_details(*info);
                    NativeEvent meta_evt{};
                    meta_evt.id = id;
                    meta_evt.kind = NativeEventKind::MetadataUpdated;
                    meta_evt.state = current_state;
                    meta_evt.name = info->name();
                    meta_evt.download_dir = status.save_path;
                    meta_evt.comment = details.comment;
                    meta_evt.source = details.source;
                    meta_evt.private_flag = details.private_flag;
                    meta_evt.has_private = details.has_private;
                    events.push_back(meta_evt);

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
                if (auto info = handle.torrent_file()) {
                    const auto details = extract_metainfo_details(*info);
                    meta.comment = details.comment;
                    meta.source = details.source;
                    meta.private_flag = details.private_flag;
                    meta.has_private = details.has_private;
                }
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
                if (!snapshot.resume_requested) {
                    handle.save_resume_data(lt::torrent_handle::save_resume_flags_t{});
                    snapshot.resume_requested = true;
                }
            }
        }

        return events;
    }

    EngineStorageState inspect_storage_state() const {
        const auto settings = session_->get_settings();
        std::uint8_t flags = 0;
        if (get_bool_setting(settings, "use_partfile", false)) {
            flags |= 0b0001;
        }
        if (get_bool_setting(settings, "coalesce_reads", true)) {
            flags |= 0b0010;
        }
        if (get_bool_setting(settings, "coalesce_writes", true)) {
            flags |= 0b0100;
        }
        if (get_bool_setting(settings, "use_disk_cache_pool", true)) {
            flags |= 0b1000;
        }

        EngineStorageState snapshot{};
        snapshot.cache_size = get_int_setting(settings, "cache_size", 0);
        snapshot.cache_expiry = get_int_setting(settings, "cache_expiry", 0);
        snapshot.flags = flags;
        snapshot.disk_read_mode = get_int_setting(settings, "disk_io_read_mode", 0);
        snapshot.disk_write_mode = get_int_setting(settings, "disk_io_write_mode", 0);
        snapshot.verify_piece_hashes = !get_bool_setting(settings, "disable_hash_checks", false);
        return snapshot;
    }

    EnginePeerClassState inspect_peer_class_state() const {
        EnginePeerClassState state{};
        state.configured_ids = rust::Vec<std::uint8_t>();
        state.default_ids = rust::Vec<std::uint8_t>();
        state.configured_ids.reserve(configured_peer_classes_.size());
        state.default_ids.reserve(default_peer_classes_.size());
        for (const auto id : configured_peer_classes_) {
            state.configured_ids.push_back(id);
        }
        for (const auto id : default_peer_classes_) {
            state.default_ids.push_back(id);
        }
        return state;
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

    struct AuthView {
        std::string username;
        std::string password;
        bool has_username{false};
        bool has_password{false};
    };

    AuthView resolve_auth_view(const TrackerAuthOptions& request) const {
        AuthView view{
            .username = request.has_username ? to_std_string(request.username) : std::string(),
            .password = request.has_password ? to_std_string(request.password) : std::string(),
            .has_username = request.has_username,
            .has_password = request.has_password,
        };

        if (!view.has_username && has_tracker_username_) {
            view.username = tracker_username_;
            view.has_username = true;
        }
        if (!view.has_password && has_tracker_password_) {
            view.password = tracker_password_;
            view.has_password = true;
        }

        return view;
    }

    static std::string percent_encode(const std::string& value) {
        std::ostringstream encoded;
        encoded << std::hex << std::uppercase;
        for (unsigned char ch : value) {
            if (std::isalnum(ch) != 0 || ch == '-' || ch == '_' || ch == '.' || ch == '~') {
                encoded << ch;
            } else {
                encoded << '%' << std::setw(2) << std::setfill('0')
                        << static_cast<int>(ch);
            }
        }
        return encoded.str();
    }

    std::string inject_basic_auth(const std::string& tracker, const AuthView& auth) const {
        const bool is_http = tracker.rfind("http://", 0) == 0;
        const bool is_https = tracker.rfind("https://", 0) == 0;
        if (!is_http && !is_https) {
            return tracker;
        }

        const auto scheme_end = tracker.find("://");
        if (scheme_end == std::string::npos) {
            return tracker;
        }

        const auto encoded_user =
            auth.has_username ? percent_encode(auth.username) : std::string();
        const auto encoded_pass =
            auth.has_password ? percent_encode(auth.password) : std::string();
        return tracker.substr(0, scheme_end + 3) + encoded_user + ":" + encoded_pass + "@"
            + tracker.substr(scheme_end + 3);
    }

    std::vector<std::string> apply_tracker_auth(
        const std::vector<std::string>& trackers,
        const AuthView& auth) const {
        if (!auth.has_username && !auth.has_password) {
            return trackers;
        }

        std::vector<std::string> rewritten;
        rewritten.reserve(trackers.size());
        for (const auto& tracker : trackers) {
            rewritten.push_back(inject_basic_auth(tracker, auth));
        }
        return rewritten;
    }

    std::unique_ptr<lt::session> session_;
    std::string default_download_root_;
    std::string resume_dir_;
    lt::storage_mode_t default_storage_mode_{lt::storage_mode_sparse};
    bool sequential_default_{false};
    std::vector<std::string> default_trackers_;
    std::vector<std::string> extra_trackers_;
    std::string tracker_username_;
    std::string tracker_password_;
    std::string tracker_cookie_;
    bool has_tracker_username_{false};
    bool has_tracker_password_{false};
    bool has_tracker_cookie_{false};
    std::array<lt::peer_class_t, 32> peer_class_map_{};
    std::vector<lt::peer_class_t> custom_peer_classes_;
    std::vector<std::uint8_t> configured_peer_classes_;
    std::vector<std::uint8_t> default_peer_classes_;
    bool replace_default_trackers_{false};
    bool announce_to_all_{false};
    bool auto_managed_default_{true};
    bool super_seeding_default_{false};
    bool pex_enabled_{true};
    int default_max_connections_per_torrent_{-1};
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

CreateTorrentResult Session::create_torrent(const CreateTorrentRequest& request) {
    return impl_->create_torrent(request);
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

::rust::String Session::update_options(const UpdateOptionsRequest& request) {
    return impl_->update_options(request);
}

::rust::String Session::update_trackers(const UpdateTrackersRequest& request) {
    return impl_->update_trackers(request);
}

::rust::String Session::update_web_seeds(const UpdateWebSeedsRequest& request) {
    return impl_->update_web_seeds(request);
}

::rust::String Session::move_torrent(const MoveTorrentRequest& request) {
    return impl_->move_torrent(request);
}

::rust::String Session::reannounce(::rust::Str id) {
    return impl_->reannounce(to_std_string(id));
}

::rust::String Session::recheck(::rust::Str id) {
    return impl_->recheck(to_std_string(id));
}

::rust::String Session::set_piece_deadline(
    ::rust::Str id, std::uint32_t piece, std::int32_t deadline_ms, bool has_deadline) {
    return impl_->set_piece_deadline(to_std_string(id), piece, deadline_ms, has_deadline);
}

rust::Vec<NativePeerInfo> Session::list_peers(::rust::Str id) {
    return impl_->list_peers(id);
}

EngineStorageState Session::inspect_storage_state() const {
    return impl_->inspect_storage_state();
}

EnginePeerClassState Session::inspect_peer_class_state() const {
    return impl_->inspect_peer_class_state();
}

rust::Vec<NativeEvent> Session::poll_events() {
    return impl_->poll_events();
}

std::unique_ptr<Session> new_session(const SessionOptions& options) {
    return std::make_unique<Session>(options);
}

}  // namespace revaer
