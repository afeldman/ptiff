/* Per-language buffer + out-parameter typemaps for the libptiff C ABI.

   The C ABI exposes tile buffers as counted (ptr, size) pairs and out-params
   as pointer args (int *err_out, uintptr_t *bytes_read). Each target language
   needs its OWN typemaps for these -- SWIG typemaps are language-specific and
   one language's typemaps do NOT apply to another (a Python Py_buffer typemap
   generated for Ruby would reference Python.h). So this file is guarded by the
   SWIG-provided per-language preprocessor macros (SWIGPYTHON / SWIGGO /
   SWIGRUBY are always defined for the matching -<lang> target).
*/

#ifdef SWIGPYTHON
/* ---- Python: buffers map onto Py_buffer (bytes/bytearray/memoryview). ---- */

/* Write side: (const uint8_t* buffer, uintptr_t buffer_size) collapses into one
   readable Python buffer; the input's length becomes buffer_size. */
%typemap(arginit) (const uint8_t* buffer, uintptr_t buffer_size) %{
  Py_buffer $1_buf;
%}
%typemap(in) (const uint8_t* buffer, uintptr_t buffer_size) %{
  if (PyObject_GetBuffer($input, &$1_buf, PyBUF_SIMPLE) < 0) {
    PyErr_SetString(PyExc_TypeError, "expected a bytes-like object");
    SWIG_fail;
  }
  $1 = (uint8_t *)$1_buf.buf;
  $2 = (uintptr_t)$1_buf.len;
%}
%typemap(freearg) (const uint8_t* buffer, uintptr_t buffer_size) %{
  PyBuffer_Release(&$1_buf);
%}

/* Read side: (uint8_t* buffer, uintptr_t buffer_size) -> one writable Python
   buffer (bytearray/memoryview); it is filled in place by the C function. */
%typemap(arginit) (uint8_t* buffer, uintptr_t buffer_size) %{
  Py_buffer $1_buf;
%}
%typemap(in) (uint8_t* buffer, uintptr_t buffer_size) %{
  if (PyObject_GetBuffer($input, &$1_buf, PyBUF_WRITABLE) < 0) {
    PyErr_SetString(PyExc_TypeError, "expected a writable bytes-like object (bytearray/memoryview)");
    SWIG_fail;
  }
  $1 = (uint8_t *)$1_buf.buf;
  $2 = (uintptr_t)$1_buf.len;
%}
%typemap(freearg) (uint8_t* buffer, uintptr_t buffer_size) %{
  PyBuffer_Release(&$1_buf);
%}

/* C ABI out-parameters surface as extra Python return values. Matched by
   (type, param name) pair, so this only touches the specific C ABI functions
   that declare a parameter with these exact names -- see the grep audit in
   the "alles in SWIG" migration notes (bindings/swig/README.md). */
%apply int *OUTPUT { int32_t *err_out };
%apply uintptr_t *OUTPUT { uintptr_t *bytes_read };
%apply double *OUTPUT { double *out };   /* ptiff_image_gsd */
%apply int *OUTPUT { int32_t *out };       /* ptiff_image_compression */
%apply int *OUTPUT { int32_t *major };       /* ptiff_{runtime,compile_time}_version_out */
%apply int *OUTPUT { int32_t *minor };
%apply int *OUTPUT { int32_t *patch };


/* ---- ptiff_open_path_fields(path, ptiff_field** out, int* out_count) ----
   Surfaces as `ptiff_open_path_fields(path)` returning a Python list of
   (key, value) string tuples. The two out-params are consumed inside the
   wrapper: `out` is filled in by the C function and `out_count` is its
   length; this typemap walks the resulting array, copies the strings into
   tuples, and frees the C array with ptiff_fields_free so the Python caller
   never sees raw pointers. The return code (0 == ok, negative == error) is
   kept as the leading return value. */
%typemap(in, numinputs=0) struct ptiff_field** out (ptiff_field* tmp) %{
  tmp = NULL;
  $1 = &tmp;
%}
%typemap(in, numinputs=0) int* out_count (int tmpcount) %{
  tmpcount = 0;
  $1 = &tmpcount;
%}
%typemap(argout) struct ptiff_field** out {
  /* arg2 == ptiff_field** out  (filled by the C function),
     arg3 == int* out_count (its length). */
  ptiff_field* farr = (arg2 != NULL) ? *arg2 : NULL;
  int n = (arg3 != NULL) ? *arg3 : 0;
  PyObject* lst = PyList_New(n > 0 ? n : 0);
  if (!lst) SWIG_fail;
  for (int i = 0; i < n; ++i) {
    PyObject* t = PyTuple_New(2);
    if (!t) { Py_DECREF(lst); SWIG_fail; }
    /* strings already null-terminated by ptiff_open_path_fields */
    PyTuple_SetItem(t, 0, SWIG_FromCharPtr(farr[i].key));
    PyTuple_SetItem(t, 1, SWIG_FromCharPtr(farr[i].value));
    PyList_SetItem(lst, i, t);
  }
  /* The signature of SWIG_Python_AppendOutput changed in SWIG 4.3.0
     (2024-06-15, commit #2907): pre-4.3 it is `(PyObject*, PyObject*)` (two
     args, no `is_void` flag); 4.3.0+ adds a third `int is_void`. Distro SWIG
     on the CI images we target (Ubuntu 24.04 -> 4.2.0, and older images ->
     4.0.x) is still pre-4.3, while the local/macOS toolchain is 4.5.0, so
     branch on SWIG_VERSION to stay buildable under both. We pass 0 (append,
     never free `lst`); the caller owns it. */
#if SWIG_VERSION >= 0x040300
  resultobj = SWIG_Python_AppendOutput(resultobj, lst, 0);
#else
  resultobj = SWIG_Python_AppendOutput(resultobj, lst);
#endif
  ptiff_fields_free(farr, n);
}

/* ---- ptiff_open_path_camera(path, ptiff_camera* out) ----
   Surfaces as `ptiff_open_path_camera(path)` returning a Python dict of the
   camera fields (scalars + K / [R|t] / P as flat lists of floats and the
   ISO-8601 timestamp string), plus the return code. The raw C struct is read
   back directly in the wrapper -- no pointer exposure. */
%typemap(in, numinputs=0) struct ptiff_camera* out (ptiff_camera cam_tmp) %{
  memset(&cam_tmp, 0, sizeof(cam_tmp));
  $1 = &cam_tmp;
%}
%typemap(argout) struct ptiff_camera* out {
  PyObject* d = PyDict_New();
  if (!d) SWIG_fail;
  PyDict_SetItemString(d, "has_intrinsics", PyLong_FromLong(cam_tmp2.has_intrinsics));
  PyDict_SetItemString(d, "has_extrinsics", PyLong_FromLong(cam_tmp2.has_extrinsics));
  PyDict_SetItemString(d, "focal_length_x", PyFloat_FromDouble(cam_tmp2.focal_length_x));
  PyDict_SetItemString(d, "focal_length_y", PyFloat_FromDouble(cam_tmp2.focal_length_y));
  PyDict_SetItemString(d, "principal_x", PyFloat_FromDouble(cam_tmp2.principal_x));
  PyDict_SetItemString(d, "principal_y", PyFloat_FromDouble(cam_tmp2.principal_y));
  PyDict_SetItemString(d, "rotation_w", PyFloat_FromDouble(cam_tmp2.rotation_w));
  PyDict_SetItemString(d, "rotation_x", PyFloat_FromDouble(cam_tmp2.rotation_x));
  PyDict_SetItemString(d, "rotation_y", PyFloat_FromDouble(cam_tmp2.rotation_y));
  PyDict_SetItemString(d, "rotation_z", PyFloat_FromDouble(cam_tmp2.rotation_z));
  PyDict_SetItemString(d, "position_x", PyFloat_FromDouble(cam_tmp2.position_x));
  PyDict_SetItemString(d, "position_y", PyFloat_FromDouble(cam_tmp2.position_y));
  PyDict_SetItemString(d, "position_z", PyFloat_FromDouble(cam_tmp2.position_z));
  {
    int i;
    PyObject* k = PyList_New(9);
    for (i = 0; i < 9; ++i) PyList_SET_ITEM(k, i, PyFloat_FromDouble(cam_tmp2.intrinsics[i]));
    PyDict_SetItemString(d, "intrinsics", k);
  }
  {
    int i;
    PyObject* e = PyList_New(12);
    for (i = 0; i < 12; ++i) PyList_SET_ITEM(e, i, PyFloat_FromDouble(cam_tmp2.extrinsics[i]));
    PyDict_SetItemString(d, "extrinsics", e);
  }
  {
    int i;
    PyObject* p = PyList_New(12);
    for (i = 0; i < 12; ++i) PyList_SET_ITEM(p, i, PyFloat_FromDouble(cam_tmp2.projection[i]));
    PyDict_SetItemString(d, "projection", p);
  }
  PyDict_SetItemString(d, "timestamp", PyUnicode_FromString((const char*)cam_tmp2.timestamp));
  /* Same SWIG_VERSION branch (>= 4.3.0) as the fields typemap above. */
#if SWIG_VERSION >= 0x040300
  resultobj = SWIG_Python_AppendOutput(resultobj, d, 0);
#else
  resultobj = SWIG_Python_AppendOutput(resultobj, d);
#endif
}


#elif defined(SWIGGO)
/* ---- Go: (ptr, size) buffer pairs collapse onto []byte. ---- */
/* Mirrors SWIG's own go/cdata.i pattern: the generated C wrapper extracts the
   slice's data pointer and length from the _goslice_ ($input.array/.len). */
%typemap(gotype) (const uint8_t* buffer, uintptr_t buffer_size) "[]byte"
%typemap(in) (const uint8_t* buffer, uintptr_t buffer_size) %{
  $1 = ($1_ltype)$input.array;
  $2 = ($2_ltype)$input.len;
%}
%typemap(gotype) (uint8_t* buffer, uintptr_t buffer_size) "[]byte"
%typemap(in) (uint8_t* buffer, uintptr_t buffer_size) %{
  $1 = ($1_ltype)$input.array;
  $2 = ($2_ltype)$input.len;
%}

/* err_out / bytes_read stay in/out *int / *int64 in Go -- the caller passes a
   pointer and reads the written value, matching Go's return-by-pointer idiom. */

#elif defined(SWIGRUBY)
/* ---- Ruby: (ptr, size) buffer pairs collapse onto a Ruby String. ---- */

%typemap(in) (const uint8_t* buffer, uintptr_t buffer_size) %{
  (void)StringValue($input);
  $1 = (uint8_t*)RSTRING_PTR($input);
  $2 = (uintptr_t)RSTRING_LEN($input);
%}

%typemap(in) (uint8_t* buffer, uintptr_t buffer_size) %{
  (void)StringValue($input);
  $1 = (uint8_t*)RSTRING_PTR($input);
  $2 = (uintptr_t)RSTRING_LEN($input);
%}

/* Ruby surfaces C out-params as extra return values via SWIG_Ruby_AppendOutput.
   Matched by (type, param name) pair -- see the grep audit in the "alles in
   SWIG" migration notes (bindings/swig/README.md). */
%apply int *OUTPUT { int32_t *err_out };
%apply uintptr_t *OUTPUT { uintptr_t *bytes_read };
%apply double *OUTPUT { double *out };   /* ptiff_image_gsd */
%apply int *OUTPUT { int32_t *out };       /* ptiff_image_compression */
%apply int *OUTPUT { int32_t *major };       /* ptiff_{runtime,compile_time}_version_out */
%apply int *OUTPUT { int32_t *minor };
%apply int *OUTPUT { int32_t *patch };

/* ---- ptiff_open_path_fields(path, ptiff_field** out, int* out_count) ----
   Surfaces as `Ptiff::ptiff_open_path_fields(path)` returning an Array of
   [key, value] string pairs (leading return code kept). Consumed the same
   way as the Python port: walk the C array, copy to Ruby strings, free. */
%typemap(in, numinputs=0) struct ptiff_field** out (ptiff_field* tmp) %{
  tmp = NULL;
  $1 = &tmp;
%}
%typemap(in, numinputs=0) int* out_count (int tmpcount) %{
  tmpcount = 0;
  $1 = &tmpcount;
%}
%typemap(argout) struct ptiff_field** out {
  ptiff_field* farr = (arg2 != NULL) ? *arg2 : NULL;
  int n = (arg3 != NULL) ? *arg3 : 0;
  VALUE arr = rb_ary_new();
  for (int i = 0; i < n; ++i) {
    VALUE pair = rb_ary_new();
    rb_ary_push(pair, rb_str_new2(farr[i].key));
    rb_ary_push(pair, rb_str_new2(farr[i].value));
    rb_ary_push(arr, pair);
  }
  /* SWIG_Ruby_AppendOutput gained a third `int is_void` in the same 4.3.0
     release as the Python form (2024-10-05, #2907); pre-4.3 distro SWIG
     (e.g. Ubuntu 24.04's 4.2.0) declares only `(VALUE, VALUE)`. Branch on
     SWIG_VERSION exactly like the Python typemaps above. We pass 0 (append;
     the caller owns `arr`, and fields are freed below). */
#if SWIG_VERSION >= 0x040300
  vresult = SWIG_Ruby_AppendOutput(vresult, arr, 0);
#else
  vresult = SWIG_Ruby_AppendOutput(vresult, arr);
#endif
  ptiff_fields_free(farr, n);
}

/* ---- ptiff_open_path_camera(path, ptiff_camera* out) ----
   Surfaces as `Ptiff::ptiff_open_path_camera(path)` returning a Ruby Hash of
   the camera fields (scalars + K / [R|t] / P as Arrays of floats and the
   ISO-8601 timestamp string), plus the return code. The populated struct is
   copied into fresh Ruby values, so no dangling pointer escapes the C frame. */
%typemap(in, numinputs=0) struct ptiff_camera* out (ptiff_camera cam_tmp) %{
  memset(&cam_tmp, 0, sizeof(cam_tmp));
  $1 = &cam_tmp;
%}
%typemap(argout) struct ptiff_camera* out {
  VALUE h = rb_hash_new();
  rb_hash_aset(h, rb_str_new2("has_intrinsics"), INT2NUM(cam_tmp2.has_intrinsics));
  rb_hash_aset(h, rb_str_new2("has_extrinsics"), INT2NUM(cam_tmp2.has_extrinsics));
  rb_hash_aset(h, rb_str_new2("focal_length_x"), DBL2NUM(cam_tmp2.focal_length_x));
  rb_hash_aset(h, rb_str_new2("focal_length_y"), DBL2NUM(cam_tmp2.focal_length_y));
  rb_hash_aset(h, rb_str_new2("principal_x"), DBL2NUM(cam_tmp2.principal_x));
  rb_hash_aset(h, rb_str_new2("principal_y"), DBL2NUM(cam_tmp2.principal_y));
  rb_hash_aset(h, rb_str_new2("rotation_w"), DBL2NUM(cam_tmp2.rotation_w));
  rb_hash_aset(h, rb_str_new2("rotation_x"), DBL2NUM(cam_tmp2.rotation_x));
  rb_hash_aset(h, rb_str_new2("rotation_y"), DBL2NUM(cam_tmp2.rotation_y));
  rb_hash_aset(h, rb_str_new2("rotation_z"), DBL2NUM(cam_tmp2.rotation_z));
  rb_hash_aset(h, rb_str_new2("position_x"), DBL2NUM(cam_tmp2.position_x));
  rb_hash_aset(h, rb_str_new2("position_y"), DBL2NUM(cam_tmp2.position_y));
  rb_hash_aset(h, rb_str_new2("position_z"), DBL2NUM(cam_tmp2.position_z));
  {
    VALUE k = rb_ary_new();
    for (int i = 0; i < 9; ++i) rb_ary_push(k, DBL2NUM(cam_tmp2.intrinsics[i]));
    rb_hash_aset(h, rb_str_new2("intrinsics"), k);
  }
  {
    VALUE e = rb_ary_new();
    for (int i = 0; i < 12; ++i) rb_ary_push(e, DBL2NUM(cam_tmp2.extrinsics[i]));
    rb_hash_aset(h, rb_str_new2("extrinsics"), e);
  }
  {
    VALUE p = rb_ary_new();
    for (int i = 0; i < 12; ++i) rb_ary_push(p, DBL2NUM(cam_tmp2.projection[i]));
    rb_hash_aset(h, rb_str_new2("projection"), p);
  }
  rb_hash_aset(h, rb_str_new2("timestamp"), rb_str_new2((const char*)cam_tmp2.timestamp));
  /* Same SWIG_VERSION branch (>= 4.3.0) as the fields typemap. */
#if SWIG_VERSION >= 0x040300
  vresult = SWIG_Ruby_AppendOutput(vresult, h, 0);
#else
  vresult = SWIG_Ruby_AppendOutput(vresult, h);
#endif
}

#elif defined(SWIGOCTAVE)
/* ---- Octave: buffers map onto uint8 arrays (or strings); read buffers are
   returned back as an extra output value, since Octave arrays are copied on
   assignment (no in-place byte mutation like a Python bytearray). ---- */

/* Write side: (const uint8_t* buffer, uintptr_t buffer_size) reads from a uint8
   array or a string; the array's length becomes buffer_size. The uint8NDArray
   copy lives in this call's scope so its data pointer stays valid.
   NOTE: SWIG names local slot variables `base4` (arg index 4 = `buffer`). */
%typemap(in) (const uint8_t* buffer, uintptr_t buffer_size) (uint8NDArray tmpwbuf) %{
  if ($input.is_string()) {
    std::string s = $input.string_value();
    $1 = reinterpret_cast<uint8_t*>(&s[0]);
    $2 = static_cast<uintptr_t>(s.size());
  } else {
    if (!$input.is_uint8_type()) {
      SWIG_exception_fail(SWIG_TypeError, "expected a uint8 array or a string");
    }
    tmpwbuf4 = $input.uint8_array_value();
    $1 = reinterpret_cast<uint8_t*>(tmpwbuf4.fortran_vec());
    $2 = static_cast<uintptr_t>(tmpwbuf4.numel());
  }
%}

/* Read side: (uint8_t* buffer, uintptr_t buffer_size) accepts a uint8 array whose
   length is taken as buffer_size (== required tile byte size). Octave copies
   arrays on assignment, so the caller can't observe in-place mutation; the
   filled data is therefore appended to the return list as a uint8 array.
   NOTE: SWIG names local slot variables `base<argindex>` (here arg index 4 for
   the `buffer` parameter of ptiff_source_read_tile) -- hence the `...4` suffix
   used below. */
%typemap(in) (uint8_t* buffer, uintptr_t buffer_size)
    (std::vector<uint8_t> tmpbuf, dim_vector dvbuf,
     uint8NDArray outbuf, uintptr_t needbuf) %{
  if (!$input.is_uint8_type() && !$input.is_string()) {
    SWIG_exception_fail(SWIG_TypeError, "expected a uint8 array or a string");
  }
  needbuf4 = static_cast<uintptr_t>($input.is_string()
      ? $input.string_value().size() : $input.uint8_array_value().numel());
  tmpbuf4.resize(needbuf4);
  $1 = tmpbuf4.data();
  $2 = static_cast<uintptr_t>(tmpbuf4.size());
%}
%typemap(argout) (uint8_t* buffer, uintptr_t buffer_size) %{
  dvbuf4 = dim_vector(1, static_cast<octave_idx_type>(tmpbuf4.size()));
  outbuf4.resize(dvbuf4);
  std::memcpy(outbuf4.fortran_vec(), tmpbuf4.data(), tmpbuf4.size());
  _outp = SWIG_Octave_AppendOutput(_outp, octave_value(outbuf4));
%}

/* Out-params surface as extra return values via SWIG_Octave_AppendOutput. */
%apply int *OUTPUT { int32_t *err_out };
%apply uintptr_t *OUTPUT { uintptr_t *bytes_read };
%apply double *OUTPUT { double *out };   /* ptiff_image_gsd */
%apply int *OUTPUT { int32_t *out };       /* ptiff_image_compression */
%apply int *OUTPUT { int32_t *major };       /* ptiff_{runtime,compile_time}_version_out */
%apply int *OUTPUT { int32_t *minor };
%apply int *OUTPUT { int32_t *patch };

/* ---- ptiff_open_path_fields(path, ptiff_field** out, int* out_count) ----
   Surfaces as `ptiff_open_path_fields(path)` returning a struct array with
   `key`/`value` string fields (leading return code kept; `rc`==0 means ok).
   Runs as C++ (SWIG-Octave runtime), so the C array is walked and freed here
   and the caller never sees raw pointers. */
%typemap(in, numinputs=0) struct ptiff_field** out
    (ptiff_field* tmp, ptiff_field* farr, octave_idx_type n,
     Cell m_keys, Cell m_vals, octave_map m_map) %{
  tmp = NULL;
  $1 = &tmp;
%}
%typemap(in, numinputs=0) int* out_count (int tmpcount) %{
  tmpcount = 0;
  $1 = &tmpcount;
%}
%typemap(argout) struct ptiff_field** out %{
  /* NOTE: SWIG numbers the `in`-typemap locals by arg index; `out` is arg 2,
     so the locals land as `farr2`/`n2`/`m_keys2`/... (same pattern the other
     Octave typemaps above use with `tmpwbuf4`/`outbuf4` for arg 4).
     m is a 1x1 struct; m.key / m.value are 1xn cell rows so the caller uses
     the idiomatic Octave accessors m.key{i} / m.value{i}. */
  farr2 = (arg2 != NULL) ? *arg2 : NULL;
  n2 = (arg3 != NULL) ? static_cast<octave_idx_type>(*arg3) : 0;
  m_keys2 = Cell(dim_vector(1, n2));
  m_vals2 = Cell(dim_vector(1, n2));
  for (octave_idx_type i = 0; i < n2; ++i) {
    m_keys2(i) = octave_value(std::string(farr2[i].key != NULL ? farr2[i].key : ""));
    m_vals2(i) = octave_value(std::string(farr2[i].value != NULL ? farr2[i].value : ""));
  }
  m_map2.clear();
  m_map2.assign("key", octave_value(m_keys2));
  m_map2.assign("value", octave_value(m_vals2));
  _outp = SWIG_Octave_AppendOutput(_outp, octave_value(m_map2));
  ptiff_fields_free(farr2, (arg3 != NULL) ? *arg3 : 0);
%}

/* ---- ptiff_open_path_camera(path, ptiff_camera* out) ----
   Surfaces as `ptiff_open_path_camera(path)` returning a struct with the
   camera fields (ints/doubles + the K / [R|t] / P row-major matrices as
   numeric vectors and the timestamp string) plus the return code. Runs as
   C++ (SWIG-Octave runtime); the populated struct is read back directly. */
%typemap(in, numinputs=0) struct ptiff_camera* out
    (ptiff_camera cam_tmp, octave_map c_map) %{
  memset(&cam_tmp, 0, sizeof(cam_tmp));
  $1 = &cam_tmp;
%}
%typemap(argout) struct ptiff_camera* out %{
  c_map2.clear();
  c_map2.setfield("has_intrinsics", octave_value(cam_tmp2.has_intrinsics));
  c_map2.setfield("has_extrinsics", octave_value(cam_tmp2.has_extrinsics));
  c_map2.setfield("focal_length_x", octave_value(cam_tmp2.focal_length_x));
  c_map2.setfield("focal_length_y", octave_value(cam_tmp2.focal_length_y));
  c_map2.setfield("principal_x", octave_value(cam_tmp2.principal_x));
  c_map2.setfield("principal_y", octave_value(cam_tmp2.principal_y));
  c_map2.setfield("rotation_w", octave_value(cam_tmp2.rotation_w));
  c_map2.setfield("rotation_x", octave_value(cam_tmp2.rotation_x));
  c_map2.setfield("rotation_y", octave_value(cam_tmp2.rotation_y));
  c_map2.setfield("rotation_z", octave_value(cam_tmp2.rotation_z));
  c_map2.setfield("position_x", octave_value(cam_tmp2.position_x));
  c_map2.setfield("position_y", octave_value(cam_tmp2.position_y));
  c_map2.setfield("position_z", octave_value(cam_tmp2.position_z));
  {
    octave_idx_type _n = 9; ColumnVector _k(_n);
    for (octave_idx_type _i = 0; _i < _n; ++_i) _k(_i) = cam_tmp2.intrinsics[_i];
    c_map2.setfield("intrinsics", octave_value(_k));
  }
  {
    octave_idx_type _n = 12; ColumnVector _e(_n);
    for (octave_idx_type _i = 0; _i < _n; ++_i) _e(_i) = cam_tmp2.extrinsics[_i];
    c_map2.setfield("extrinsics", octave_value(_e));
  }
  {
    octave_idx_type _n = 12; ColumnVector _p(_n);
    for (octave_idx_type _i = 0; _i < _n; ++_i) _p(_i) = cam_tmp2.projection[_i];
    c_map2.setfield("projection", octave_value(_p));
  }
  c_map2.setfield("timestamp", octave_value(std::string(reinterpret_cast<const char*>(cam_tmp2.timestamp))));
  _outp = SWIG_Octave_AppendOutput(_outp, octave_value(c_map2));
%}

#endif
