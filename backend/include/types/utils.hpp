#pragma once

#include <memory>
#include <tuple>
#include <type_traits>

template <typename> struct is_tuple : std::false_type {};

template <typename... T> struct is_tuple<std::tuple<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_tuple_v = is_tuple<T>::value;

template <typename> struct is_shared_ptr : std::false_type {};

template <typename... T>
struct is_shared_ptr<std::shared_ptr<T...>> : std::true_type {};

template <typename T>
inline constexpr bool is_shared_ptr_v = is_shared_ptr<T>::value;

template <typename T> struct remove_shared_ptr { using type = T; };

template <typename T> struct remove_shared_ptr<std::shared_ptr<T>> {
    using type = T;
};

template <typename T>
using remove_shared_ptr_t = typename remove_shared_ptr<T>::type;

template <typename Source, typename Target, std::size_t... Is>
auto create_references_helper(const Source &t, std::index_sequence<Is...>) {
    return std::make_tuple([&]() {
        using TargetElementType = std::tuple_element_t<Is, Target>;
        using SourceElementType = std::tuple_element_t<Is, Source>;

        if constexpr (std::is_same_v<
                          std::decay_t<TargetElementType>,
                          std::shared_ptr<std::decay_t<SourceElementType>>>) {
            return std::make_shared<std::decay_t<SourceElementType>>(
                std::get<Is>(t));
        } else {
            return std::get<Is>(t);
        }
    }()...);
}

template <typename Target, typename Source>
auto create_references(const Source &t) {
    if constexpr (is_tuple_v<Source>) {
        return create_references_helper<Source, Target>(
            t, std::make_index_sequence<std::tuple_size_v<Source>>());
    } else {
        return t;
    }
}

template <typename Tuple, std::size_t... Is>
auto destroy_references_helper(const Tuple &t, std::index_sequence<Is...>) {
    return std::make_tuple([&]() {
        using ElementType = std::tuple_element_t<Is, Tuple>;

        if constexpr (std::is_same_v<ElementType,
                                     remove_shared_ptr_t<ElementType>>) {
            return std::get<Is>(t);
        } else {
            return *std::get<Is>(t);
        }
    }()...);
}

template <typename T> auto destroy_references(const T &t) {
    if constexpr (is_tuple_v<T>) {
        return destroy_references_helper<T>(
            t, std::make_index_sequence<std::tuple_size_v<T>>());
    } else {
        return t;
    }
}

template <typename T> struct destroy_references_struct { using type = T; };

template <typename... Ts> struct destroy_references_struct<TupleT<Ts...>> {
    using type = TupleT<remove_shared_ptr_t<Ts>...>;
};

template <typename T>
using destroy_references_t = typename destroy_references_struct<T>::type;

template <typename U, typename T> U dynamic_fn_cast(T f) {
    return std::dynamic_pointer_cast<remove_shared_ptr_t<U>>(f);
}
