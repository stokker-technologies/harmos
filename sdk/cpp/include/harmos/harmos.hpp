/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#pragma once

/* === Imports === */

// Standard Library Imports
#include <chrono>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <tuple>
#include <typeindex>
#include <type_traits>
#include <unordered_map>
#include <utility>
#include <vector>

// Third Party Imports
#include <nlohmann/json.hpp>
#include <nanoarrow/nanoarrow.hpp>

// Local Imports
#include <harmos.h>


/* === Macros === */

#define HARMOS_MEMBER(T, name) ::harmos::field(#name, &T::name)
#define HARMOS_FIELDS_1(T, a) HARMOS_MEMBER(T, a)
#define HARMOS_FIELDS_2(T, a, b) HARMOS_FIELDS_1(T, a), HARMOS_MEMBER(T, b)
#define HARMOS_FIELDS_3(T, a, b, c) HARMOS_FIELDS_2(T, a, b), HARMOS_MEMBER(T, c)
#define HARMOS_FIELDS_4(T, a, b, c, d) HARMOS_FIELDS_3(T, a, b, c), HARMOS_MEMBER(T, d)
#define HARMOS_FIELDS_5(T, a, b, c, d, e) HARMOS_FIELDS_4(T, a, b, c, d), HARMOS_MEMBER(T, e)
#define HARMOS_FIELDS_6(T, a, b, c, d, e, f) HARMOS_FIELDS_5(T, a, b, c, d, e), HARMOS_MEMBER(T, f)
#define HARMOS_FIELDS_7(T, a, b, c, d, e, f, g) HARMOS_FIELDS_6(T, a, b, c, d, e, f), HARMOS_MEMBER(T, g)
#define HARMOS_FIELDS_8(T, a, b, c, d, e, f, g, h) HARMOS_FIELDS_7(T, a, b, c, d, e, f, g), HARMOS_MEMBER(T, h)
#define HARMOS_SELECT(_1,_2,_3,_4,_5,_6,_7,_8,NAME,...) NAME
#define HARMOS_FIELDS(T, ...) \
    template<> struct harmos::Fields<T> { \
        static constexpr auto members = std::tuple{HARMOS_SELECT(__VA_ARGS__, HARMOS_FIELDS_8, HARMOS_FIELDS_7, HARMOS_FIELDS_6, \
            HARMOS_FIELDS_5, HARMOS_FIELDS_4, HARMOS_FIELDS_3, HARMOS_FIELDS_2, HARMOS_FIELDS_1)(T, __VA_ARGS__)}; \
    };


/* === Constants === */

/* === Types === */

namespace harmos {

template<class T> struct Fields;

template<class T, class M> struct Field {
    const char* name;
    M T::* member;
};

template<class T, class M>
constexpr auto field(const char* name, M T::* member) { return Field<T, M>{name, member}; }

struct Policy {
    std::chrono::milliseconds timeout;
    std::chrono::milliseconds attempt_timeout;
    std::chrono::milliseconds cancellation_timeout{2000};
};

struct Stream {
    const char* name;
    std::size_t window;
};

struct Batch {
    nanoarrow::UniqueSchema schema;
    nanoarrow::UniqueArray array;
};

class Cancelled : public std::runtime_error {
public:
    Cancelled() : std::runtime_error("operation cancelled") {}
};

namespace detail {
using Json = nlohmann::json;

template<class T> concept Reflected = requires { Fields<T>::members; };
template<class T> struct Optional : std::false_type {};
template<class T> struct Optional<std::optional<T>> : std::true_type { using Value = T; };
template<class T> struct Repeated : std::false_type {};
template<class T, class A> struct Repeated<std::vector<T, A>> : std::true_type { using Value = T; };
template<class T> inline constexpr bool bytes = std::is_same_v<T, std::vector<std::uint8_t>>;

template<class T> Json encode(const T& value) {
    if constexpr (Reflected<T>) {
        auto result = Json::object();
        std::apply([&](auto... fields) { ((result[fields.name] = encode(value.*fields.member)), ...); }, Fields<T>::members);
        return result;
    } else if constexpr (Optional<T>::value) {
        return value ? encode(*value) : Json(nullptr);
    } else if constexpr (Repeated<T>::value) {
        auto result = Json::array();
        for (const auto& item : value) result.push_back(encode(item));
        return result;
    } else { return Json(value); }
}

template<class T> T decode(const Json& value) {
    if constexpr (Reflected<T>) {
        T result{};
        std::apply([&](auto... fields) {
            ((result.*fields.member = decode<std::remove_cvref_t<decltype(result.*fields.member)>>(value.at(fields.name))), ...);
        }, Fields<T>::members);
        return result;
    } else if constexpr (Optional<T>::value) {
        return value.is_null() ? T{} : T{decode<typename Optional<T>::Value>(value)};
    } else if constexpr (Repeated<T>::value) {
        T result;
        for (const auto& item : value) result.push_back(decode<typename Repeated<T>::Value>(item));
        return result;
    } else { return value.template get<T>(); }
}

class Schema {
    Json definitions = Json::array({Json::array()});
    std::unordered_map<std::type_index, std::size_t> registered;

    template<class T> Json kind() {
        if constexpr (std::is_same_v<T, bool>) return {{"type", "bool"}};
        else if constexpr (std::is_same_v<T, std::int32_t>) return {{"type", "i32"}};
        else if constexpr (std::is_same_v<T, std::int64_t>) return {{"type", "i64"}};
        else if constexpr (std::is_same_v<T, std::uint32_t>) return {{"type", "u32"}};
        else if constexpr (std::is_same_v<T, std::uint64_t>) return {{"type", "u64"}};
        else if constexpr (std::is_same_v<T, float>) return {{"type", "f32"}};
        else if constexpr (std::is_same_v<T, double>) return {{"type", "f64"}};
        else if constexpr (std::is_same_v<T, std::string>) return {{"type", "string"}};
        else if constexpr (bytes<T>) return {{"type", "bytes"}};
        else { static_assert(Reflected<T>, "declare this type with HARMOS_FIELDS"); return {{"type", "message"}, {"definition", definition<T>()}}; }
    }

    template<class T, class M> Json member(Field<T, M> field) {
        const char* cardinality = "required";
        Json shape;
        if constexpr (Optional<M>::value) { cardinality = "optional"; shape = kind<typename Optional<M>::Value>(); }
        else if constexpr (Repeated<M>::value && !bytes<M>) { cardinality = "repeated"; shape = kind<typename Repeated<M>::Value>(); }
        else shape = kind<M>();
        const auto tag = hrm_schema_field_tag({reinterpret_cast<const std::uint8_t*>(field.name), std::char_traits<char>::length(field.name)});
        if (tag == 0) throw std::invalid_argument("invalid field name");
        return {{"name", field.name}, {"tag", tag}, {"kind", std::move(shape)}, {"cardinality", cardinality}};
    }

    template<class T> std::size_t definition() {
        auto found = registered.find(typeid(T));
        if (found != registered.end()) return found->second;
        const auto index = definitions.size();
        registered.emplace(typeid(T), index);
        definitions.push_back(Json::array());
        auto fields = Json::array();
        std::apply([&](auto... entries) { (fields.push_back(member(entries)), ...); }, Fields<T>::members);
        definitions[index] = std::move(fields);
        return index;
    }
public:
    template<class O> Json contract() {
        const auto request = definition<O>();
        const auto response = definition<typename O::Response>();
        definitions[0] = Json::array({
            {{"name", "request"}, {"tag", 1}, {"kind", {{"type", "message"}, {"definition", request}}}, {"cardinality", "required"}},
            {{"name", "response"}, {"tag", 2}, {"kind", {{"type", "message"}, {"definition", response}}}, {"cardinality", "required"}},
        });
        return {{"root", 0}, {"definitions", std::move(definitions)}};
    }
};

struct Error {
    HrmError value{};
    ~Error() { hrm_error_free(&value); }
    void check(int status) const {
        if (status != 0) throw std::runtime_error(std::string(reinterpret_cast<const char*>(value.message.data), value.message.len));
    }
};
struct Buffer {
    HrmBuffer value{};
    ~Buffer() { hrm_buffer_free(&value); }
};
inline HrmSlice slice(std::string_view text) { return {reinterpret_cast<const std::uint8_t*>(text.data()), text.size()}; }
inline void callback_error(HrmError* error, std::string_view message) noexcept {
    HrmError allocation{};
    hrm_buffer_copy(slice(message), &error->message, &allocation);
    error->code = 1;
    hrm_error_free(&allocation);
}
} // namespace detail


/* === Classes === */

class Context {
    HrmContext* context;
public:
    explicit Context(HrmContext* context) : context(context) {}

    void check_cancelled() const {
        if (hrm_context_cancelled(context) != 0) throw Cancelled{};
    }

    template<class O> typename O::Response operation(const O& request) {
        const auto bytes = detail::encode(request).dump();
        detail::Buffer response;
        detail::Error error;
        error.check(hrm_context_operation(context, O::id, detail::slice(bytes), &response.value, &error.value));
        const auto value = detail::Json::parse(response.value.data, response.value.data + response.value.len);
        return detail::decode<typename O::Response>(value);
    }

    void publish(const char* stream, Batch& batch) {
        detail::Error error;
        error.check(hrm_context_publish_arrow(context, stream, batch.array.get(), batch.schema.get(), &error.value));
    }
};

class Plugin {
    HrmPlugin* plugin;

    template<class O> static int run(void*, HrmContext* native, HrmSlice input, HrmBuffer* output, HrmError* error) noexcept {
        try {
            auto request = detail::decode<O>(detail::Json::parse(input.data, input.data + input.len));
            Context context{native};
            const auto result = request.run(context);
            const auto bytes = detail::encode(result).dump();
            return hrm_buffer_copy(detail::slice(bytes), output, error);
        } catch (const std::exception& failure) { detail::callback_error(error, failure.what()); }
        catch (...) { detail::callback_error(error, "unknown C++ exception"); }
        return 1;
    }
public:
    explicit Plugin(const char* id, std::size_t capacity = 16) {
        detail::Error error;
        plugin = hrm_plugin_new(id, capacity, &error.value);
        error.check(plugin ? 0 : 1);
    }
    ~Plugin() { hrm_plugin_free(plugin); }
    Plugin(const Plugin&) = delete;
    Plugin& operator=(const Plugin&) = delete;
    Plugin(Plugin&& other) noexcept : plugin(std::exchange(other.plugin, nullptr)) {}
    Plugin& operator=(Plugin&&) = delete;

    template<class O> void operation() {
        const auto schema = detail::Schema{}.contract<O>().dump();
        const auto policy = O::policy;
        if (policy.timeout.count() <= 0 || policy.attempt_timeout.count() <= 0 || policy.cancellation_timeout.count() < 0)
            throw std::invalid_argument("invalid operation policy");
        HrmPolicy native{static_cast<std::uint64_t>(policy.timeout.count()), static_cast<std::uint64_t>(policy.attempt_timeout.count()),
            static_cast<std::uint64_t>(policy.cancellation_timeout.count())};
        detail::Error error;
        error.check(hrm_plugin_operation(plugin, O::id, detail::slice(schema), native, &run<O>, nullptr, &error.value));
        if constexpr (requires { O::streams; }) {
            for (const auto& stream : O::streams) {
                detail::Error error;
                error.check(hrm_plugin_stream(plugin, stream.name, stream.window, &error.value));
            }
        }
    }

    int serve() {
        detail::Error error;
        error.check(hrm_plugin_serve(plugin, &error.value));
        return 0;
    }
};

} // namespace harmos


/* === Templates === */

/* === Public Declarations === */
