API Reference
=============

``ptiff`` is promoted SWIG output (see the "alles in SWIG" migration notes in
``bindings/swig/README.md``, section "Planned migration"): this is the raw
C-ABI surface SWIG generates from ``bindings/swig/ptiff.i``,
not a hand-written idiomatic wrapper. Functions and types are named and
shaped exactly as the C ABI declares them (``ptiff_image_create``,
``PTIFF_PIXEL_UINT16``, ``ptiff_image_descriptor`` as a struct with public
fields, ...), with no Pythonic error type or builder API on top.

.. automodule:: ptiff
   :members:
   :undoc-members:
   :show-inheritance:
   :special-members: __init__
   :exclude-members: __dict__, __weakref__
