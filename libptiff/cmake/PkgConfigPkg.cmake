# PkgConfigPkg.cmake
#
# Helper that renders a pkg-config template (<name>.pc.in) into a real .pc in
# the build dir and registers it for installation. Consumed by both libptiff and
# bindings/c so their .pc files share one, consistent, relocatable layout.
#
# Signature:
#   ptiff_pkg_config(<name> <template.pc.in>)
#
# The generated .pc is relocatable: the template sets `prefix` from `${pcfiledir}`
# (the `.pc` lives at <prefix>/<libdir>/pkgconfig, i.e. two levels below prefix),
# so the file resolves correct paths no matter where the tree is installed --
# including `cmake --install --prefix <elsewhere>` overrides.
#
# The template should substitute @PKGCONFIG_LIBDIR@ and @PKGCONFIG_INCLUDEDIR@
# with the *relative* (to prefix) install dirs, e.g. "lib" / "include".

function(ptiff_pkg_config NAME TEMPLATE)
    include(GNUInstallDirs)

    # Keep install dirs as prefix-relative paths (fall back to the well-known
    # "lib" / "include" when GNUInstallDirs handed back an absolute path).
    set(_libdir "${CMAKE_INSTALL_LIBDIR}")
    set(_includedir "${CMAKE_INSTALL_INCLUDEDIR}")
    if(IS_ABSOLUTE "${_libdir}")
        set(_libdir "lib")
    endif()
    if(IS_ABSOLUTE "${_includedir}")
        set(_includedir "include")
    endif()

    set(PKGCONFIG_LIBDIR     "${_libdir}")
    set(PKGCONFIG_INCLUDEDIR "${_includedir}")

    set(_pc  "${CMAKE_CURRENT_BINARY_DIR}/${NAME}.pc")
    configure_file("${TEMPLATE}" "${_pc}" @ONLY)
    install(FILES "${_pc}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")

    message(STATUS "${NAME}: generating pkg-config file -> \${CMAKE_INSTALL_LIBDIR}/pkgconfig/${NAME}.pc")
endfunction()
