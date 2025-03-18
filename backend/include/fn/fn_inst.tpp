#pragma once

#include "fn/fn_inst.hpp"
#include "fn_inst.hpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"
#include "work/work.tpp"

#include <memory>
#include <map>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::TypedFnI() = default;

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::~TypedFnI() = default;

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::TypedFnI(const ArgsT &args) : args(args){};

template <typename Ret, typename... Args>
typename TypedFnI<Ret, Args...>::RetT TypedFnI<Ret, Args...>::run() {
    return std::apply([this](auto &...t) { return body(t...); }, this->args);
}

template <typename Ret, typename... Args>
void TypedFnI<Ret, Args...>::process(WorkT &work) const {
    if (execute_immediately()) {
        work->run();
    }
}

template <typename T> auto convert_value(T &&arg) {
    using U = std::decay_t<T>;
    if constexpr (is_lazy_v<U>) {
        if constexpr (std::is_same_v<remove_lazy_t<U>, Int> ||
                      std::is_same_v<remove_lazy_t<U>, Bool>) {
            if (arg->done()) {
                return std::make_tuple(Int(arg->value()));
            }
        }
        return std::make_tuple(std::bit_cast<void *>(arg->get()));
    } else if constexpr (std::is_same_v<U, Int> || std::is_same_v<U, Bool>) {
        return std::make_tuple(Int(arg));
    } else if constexpr (is_shared_ptr_v<U>) {
        return std::make_tuple(std::bit_cast<void *>(arg.get()));
    } else {
        return std::make_tuple(std::bit_cast<void *>(arg));
    }
}

template <typename... Ts> auto convert_value(std::tuple<Ts...> &&arg) {
    return std::apply(
        [](auto &&...vs) { return std::tuple_cat(convert_value(vs)...); }, arg);
}

auto convert_key(auto &...args) {
    return std::apply(
        [](auto &&...vs) {
            return std::vector<std::variant<Int, void *>>{vs...};
        },
        convert_value(std::make_tuple(args...)));
}

template <typename Ret, typename... Args>
template <typename R, typename... As, typename... AT>
    requires(std::is_same_v<As, remove_lazy_t<std::decay_t<AT>>> && ...)
LazyT<R>
TypedFnI<Ret, Args...>::fn_call(FnT<R, As...> f, AT... args) {
    if constexpr (is_tuple_v<R>){
        auto [work, res] = Work::fn_call(f, args...);
        process(work);
        return res;
    } else {
        auto key = convert_key(f, args...);
        if (cache.contains(key)) {
            return std::dynamic_pointer_cast<remove_shared_ptr_t<LazyT<R>>>(cache.at(key));
        } else {
            auto [work, res] = Work::fn_call(f, args...);
            cache.insert_or_assign(key, res);
            process(work);
            return res;
        }
    }
}

template <typename Ret, typename... Args>
constexpr bool TypedFnI<Ret, Args...>::execute_immediately() const {
    return !is_recursive() &&
           upper_size_bound() < IMMEDIATE_EXECUTION_THRESHOLD;
}

template <typename Ret, typename... Args>
void TypedFnI<Ret, Args...>::set_fn(
    const std::shared_ptr<TypedFnG<Ret, Args...>> &fn) {}

template <typename E, typename Ret, typename... Args>
TypedClosureI<E, Ret, Args...>::TypedClosureI(const ArgsT &args,
                                              const EnvT &env)
    : TypedFnI<Ret, Args...>(args), env(env) {}

template <typename E, typename Ret, typename... Args>
void TypedClosureI<E, Ret, Args...>::set_fn(
    const std::shared_ptr<TypedFnG<Ret, Args...>> &f) {
    fn = f;
}
