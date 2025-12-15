#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include "rust/cxx.h"

namespace revaer {

struct SessionOptions;
struct EngineOptions;
struct AddTorrentRequest;
struct LimitRequest;
struct UpdateOptionsRequest;
struct SelectionRules;
struct NativeEvent;

class Session {
public:
    explicit Session(const SessionOptions& options);
    ~Session();

    ::rust::String apply_engine_profile(const EngineOptions& options);
    ::rust::String add_torrent(const AddTorrentRequest& request);
    ::rust::String remove_torrent(::rust::Str id, bool with_data);
    ::rust::String pause_torrent(::rust::Str id);
    ::rust::String resume_torrent(::rust::Str id);
    ::rust::String set_sequential(::rust::Str id, bool sequential);
    ::rust::String load_fastresume(::rust::Str id, rust::Slice<const std::uint8_t> data);
    ::rust::String update_limits(const LimitRequest& request);
    ::rust::String update_selection(const SelectionRules& request);
    ::rust::String update_options(const UpdateOptionsRequest& request);
    ::rust::String reannounce(::rust::Str id);
    ::rust::String recheck(::rust::Str id);
    rust::Vec<NativeEvent> poll_events();

private:
    class Impl;
    std::unique_ptr<Impl> impl_;
};

std::unique_ptr<Session> new_session(const SessionOptions& options);

}  // namespace revaer
