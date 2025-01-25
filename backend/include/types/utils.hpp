#pragma once

#include <tuple>
#include <type_traits>

template <typename> struct is_tuple : std::false_type {};

template <typename... T> struct is_tuple<std::tuple<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_tuple_v = is_tuple<T>::value;

template <typename Source, typename Target, std::size_t... Is>
auto create_references_helper(const Source &t, std::index_sequence<Is...>) {
    return std::make_tuple([&]() {
        using TargetElementType = std::tuple_element_t<Is, Target>;
        using SourceElementType = std::tuple_element_t<Is, Source>;

        if constexpr (std::is_same_v<std::decay_t<TargetElementType>,
                                     std::add_pointer_t<
                                         std::decay_t<SourceElementType>>>) {
            return new std::decay_t<SourceElementType>(std::get<Is>(t));
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
                                     std::remove_pointer_t<ElementType>>) {
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
    using type = TupleT<std::remove_pointer_t<Ts>...>;
};

template <typename T>
using destroy_references_t = typename destroy_references_struct<T>::type;
