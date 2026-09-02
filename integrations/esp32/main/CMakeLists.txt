idf_component_register(
    SRCS
        "main.c"
        "../../../components/expanse/test/bench_esp32_memtable.c"
        "../../../components/expanse/test/test_expanse.c"
        "../../../components/expanse/test/twin_containers.c"
    INCLUDE_DIRS
        "."
    PRIV_REQUIRES
        expanse
        unity
        esp_system
        esp_hw_support
        heap
    # test_expanse.c's Unity cases register from constructor helpers and the
    # file exports nothing anyone links against, so without WHOLE_ARCHIVE the
    # linker never extracts that member from libmain.a: the cases vanish and
    # Unity prints "0 Tests ... OK". main.c asserts the case count for the
    # same reason (#579).
    WHOLE_ARCHIVE
)
