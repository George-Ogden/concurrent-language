#include "main/main.hpp"
#include "system/work_manager.hpp"
#include "time/utils.hpp"
#include "types/utils.hpp"

#include <iostream>

int main(int argc, char *argv[]) {
    auto start = time_utils::now();
    using ArgsT = typename Main::ArgsT;
    argc--;
    argv++;
    constexpr auto N = std::tuple_size_v<ArgsT>;
    if (N != argc) {
        std::cerr << "Invalid number of arguments expected " << N << " got "
                  << argc << "." << std::endl;
        exit(1);
    }

    ArgsT args = [&argv]<std::size_t... Is>(std::index_sequence<Is...>) {
        return std::make_tuple(
            make_lazy<remove_lazy_t<std::tuple_element_t<Is, ArgsT>>>(
                convert_arg<remove_lazy_t<std::tuple_element_t<Is, ArgsT>>>(
                    argv[Is]))...);
    }
    (std::make_index_sequence<N>{});

    function_equivalent_t<Main> main{Main::init};
    auto result = std::apply(
        [&main](auto &...args) { return WorkManager::run(main, args...); },
        args);

    auto end = time_utils::now();

    std::cout << result << std::endl;

    auto duration = time_utils::time_delta(start, end);
    std::cerr << "Execution time: " << duration << std::endl;

    return 0;
}
