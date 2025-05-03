#!/usr/bin/env python
# -*- coding: utf-8 -*-
#~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

import sys
import os
import platform
import ctypes
import time
import traceback

try:
  import tkinter as tk # python3
except:
  import Tkinter as tk # python2

#~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

if str is bytes: # python2
  from collections import MutableMapping
else: # python3
  from collections.abc import MutableMapping
class Record(MutableMapping):
  __init__   =lambda self, *args, **kw: self.__dict__.update(*args, **kw)
  __repr__   =lambda self:              self.__dict__.__repr__()
  __getitem__=lambda self, key:         self.__dict__.__getitem__(key)
  __setitem__=lambda self, key, item:   self.__dict__.__setitem__(key, item)
  __delitem__=lambda self, key:         self.__dict__.__delitem__(key)
  __iter__   =lambda self:              self.__dict__.__iter__()
  __len__    =lambda self:              self.__dict__.__len__()

def txt(fmt, *args, **kw):
  return fmt.format(*args, **kw)

def out(fmt, *args, **kw):
  sys.stdout.write(txt(fmt, *args, **kw))

def err(fmt, *args, **kw):
  sys.stderr.write(txt(fmt, *args, **kw))

#~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

# ensure consistent translations amongst various systems
key_trans={
  '\r': '\n',
  '\x1b': 'Escape'}

# keep track of KP because sometimes KR determines incorrect keysyms
key_cache={}

def event_info(app, kind, evt):
  e=Record(
    x=app.lbl.winfo_pointerx()-app.lbl.winfo_rootx(),
    y=app.lbl.winfo_pointery()-app.lbl.winfo_rooty(),
    w=app.lbl.winfo_width(),
    h=app.lbl.winfo_height(),
    kind=kind, btn=0, key=b'')
  if e.kind==b'W':
    e.kind=b'BP' # translate wheel-up/down to button-press-4/5
    e.btn=4 if evt.delta<0 else 5
  elif e.kind==b'KP':
    e.key=evt.char if evt.char else evt.keysym
    e.key=key_trans.get(e.key, e.key)
    key_cache[(evt.state, evt.keycode)]=e.key
  elif e.kind==b'KR':
    e.key=key_cache.get((evt.state, evt.keycode),
      evt.char if evt.char else evt.keysym)
  elif e.kind==b'BP':
    e.btn=evt.num
  elif e.kind==b'BR':
    e.btn=evt.num
    if e.btn>3: return None # no release for mouse-wheel
  if type(e.key) is not bytes: e.key=e.key.encode(sys.stdout.encoding)
  return e

def make_screen(app):
  byte_count=3*app.width*app.height
  alignment=64 # align data on cacheline
  pfx1=b'P6\n'
  pfx2=txt('{} {}\n255\n', app.width, app.height).encode()
  offset=len(pfx1)+len(pfx2)
  app.ppm=bytearray(offset+alignment+byte_count)
  addr=ctypes.addressof((ctypes.c_uint8*len(app.ppm)).from_buffer(app.ppm))
  adjust=(addr+offset)%alignment
  if adjust: adjust=alignment-adjust
  pfx=pfx1+(b' '*adjust)+pfx2
  offset+=adjust
  for i in range(offset): app.ppm[i]=pfx[i]
  app.screen=(ctypes.c_uint8*byte_count).from_buffer(app.ppm, offset)

def update_ppm(app):
  if app.ppm_convert is None:
    # depending on python/tkinter versions (and bugs), some conversions
    # are required when providing Tkinter.PhotoImage data
    # - python 3 wants bytes() and directly accesses them
    # - python 2 wants utf-8 encoded str() but decodes it to bytes again!
    app.ppm_convert=[] # one of these should work...
    if 0: # memory leak!!!
      app.ppm_convert.append(lambda ppm: app.top.tk._createbytearray(ppm))
    if tk.__name__=='tkinter': # bad utf-8 conversions in python 2
      app.ppm_convert.append(lambda ppm: bytes(ppm))
    app.ppm_convert.append(lambda ppm: ppm.decode('latin-1').encode('utf-8'))
    app.ppm_convert.append(lambda ppm: ppm.decode('latin-1').encode('utf-8')
                                          .replace(b'\x00', b'\xc0\x80'))
  while app.ppm_convert:
    try:
      ppm=app.ppm_convert[0](app.ppm)
      # FIXME: some versions of Tkinter.PhotoImage have a memory leak!
      #        (probably in tk._createbytearray)
      #        thus we perform a direct call to Tcl/Tk
      if 0:
        app.photo.config(width=app.width, height=app.height,
                         format='ppm', data=ppm)
      else:
        app.top.tk.call(app.photo.name, 'configure',
                        '-width', app.width, '-height', app.height,
                        '-format', 'ppm', '-data', ppm)
      break # this conversion was suitable
    except:
      # traceback.print_exc()
      del app.ppm_convert[0] # forget this conversion which was not suitable

def callback(app, kind):
  return lambda *evt_args: handle_event(evt_args, app, kind)

def handle_event(evt_args, app, kind):
  if app.must_quit: return
  evt=event_info(app, kind, evt_args[0] if evt_args else None)
  if evt is None: return # ignore event
  if evt.w!=app.width or evt.h!=app.height or app.screen is None:
    (app.width, app.height)=(evt.w, evt.h)
    make_screen(app)
  now=time.time()
  result=app.ntv_update(
    evt.kind, evt.x, evt.y, evt.w, evt.h, evt.btn, evt.key,
    app.screen, app.ntv_state)
  if result<0:
    app.must_quit=True
    tk._default_root.quit()
  else:
    if result&1:
      update_ppm(app)
      tk._default_root.update_idletasks() # make visible ASAP
    if evt.kind==b'T':
      dt_ms=int(1000.0*(now+app.dt-time.time()))
      app.timeout_id=app.top.after(max(1, dt_ms), callback(app, b'T'))

def make_gui(app):
  if not tk._default_root: tk.Tk().withdraw()
  app.top=tk.Toplevel()
  app.top.title('NtvPy -- %s'%sys.argv[1])
  app.top.config(borderwidth=0, padx=0, pady=0)
  app.top.rowconfigure(0, weight=1)
  app.top.columnconfigure(0, weight=1)
  app.photo=tk.PhotoImage(width=app.width, height=app.height)
  app.lbl=tk.Label(app.top)
  app.lbl.config(width=app.width, height=app.height,
                 anchor=tk.NW, image=app.photo)
  app.lbl.grid(row=0, column=0, sticky=tk.NSEW)
  # ugly way to force exact initial window size
  (w_req, h_req)=(app.width, app.height)
  for i in range(1000): # don't get stuck forever!
    tk._default_root.update()
    (dw, dh)=(app.lbl.winfo_width()-app.width,
              app.lbl.winfo_height()-app.height)
    if not dw and not dh: break
    w_req+=1 if dw<0 else (-1 if dw>0 else 0)
    h_req+=1 if dh<0 else (-1 if dh>0 else 0)
    app.lbl.config(width=w_req, height=h_req)
  #
  app.ppm_convert=None
  app.ppm=None
  app.screen=None
  app.top.wm_protocol('WM_DELETE_WINDOW', callback(app, b'Q'))
  app.top.bind('<Motion>', callback(app, b'M'))
  app.top.bind('<MouseWheel>', callback(app, b'W'))
  app.top.bind('<ButtonPress>', callback(app, b'BP'))
  app.top.bind('<ButtonRelease>', callback(app, b'BR'))
  app.top.bind('<KeyPress>', callback(app, b'KP'))
  app.top.bind('<KeyRelease>', callback(app, b'KR'))
  app.lbl.bind('<Configure>', callback(app, b'C'))
  app.lbl.event_generate('<Configure>', when='tail') # ensure opening is seen
  if app.dt>=0.0:
    dt_ms=int(1000.0*app.dt)
    app.timeout_id=app.top.after(max(1, dt_ms), callback(app, b'T'))

#~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

def load_native_functions(app, argv):
  if len(argv)<2:
    err('usage: {} library_name [ args... ]\n', argv[0])
    return False
  libname=argv[1]
  paths=[os.path.curdir, os.path.dirname(sys.modules[__name__].__file__)]
  if sys.platform=='win32':
    sys_libname=txt('{}.dll', libname)
    if hasattr(os, 'add_dll_directory'):
      # starting from Python 3.8, PATH is ignored when loading DLLs
      for p in os.environ['PATH'].split(';'):
        if p and p not in paths and os.path.exists(p):
          paths.append(p)
  elif sys.platform=='darwin':
    sys_libname=txt('lib{}.dylib', libname)
  else:
    sys_libname=txt('lib{}.so', libname)
  app.ntv_lib=None
  paths.append('')
  for d in paths:
    path=os.path.abspath(os.path.join(d, sys_libname)) if d else sys_libname
    try:
      app.ntv_lib=ctypes.CDLL(path)
      break
    except:
      # traceback.print_exc()
      pass
  if app.ntv_lib is None:
    err('cannot load library {!r}\n', libname)
    return None
  for n in ('_init', '_update'):
    fnct_name=libname+n
    try:
      fnct=app.ntv_lib[fnct_name]
    except:
      traceback.print_exc()
      err('cannot find {!r} in {!r}\n', fnct_name, libname)
      return None
    app['ntv'+n]=fnct
  #
  app.ntv_init.restype=ctypes.c_void_p
  app.ntv_init.argtypes=[ctypes.c_int,    # argc
                         ctypes.c_void_p, # argv
                         ctypes.c_void_p, # &inout_width
                         ctypes.c_void_p, # &inout_height
                         ctypes.c_void_p] # &inout_dt
  #
  app.ntv_update.restype=ctypes.c_int
  app.ntv_update.argtypes=[ctypes.c_char_p, # evt
                           ctypes.c_int,    # x
                           ctypes.c_int,    # y
                           ctypes.c_int,    # w
                           ctypes.c_int,    # h
                           ctypes.c_int,    # btn
                           ctypes.c_char_p, # key
                           ctypes.c_void_p, # screen
                           ctypes.c_void_p] # app
  return True

def main(argv):
  app=Record()
  if not load_native_functions(app, argv):
    return 1
  #
  args=(ctypes.c_char_p*(len(argv)+1))()
  for (i, a) in enumerate(argv):
    args[i]=a.encode(sys.stdout.encoding) if bytes!=str else a # python 2
  (width, height)=(ctypes.c_int(640), ctypes.c_int(480))
  dt=ctypes.c_double(-1.0)
  app.ntv_state=app.ntv_init(len(argv), args,
                             ctypes.byref(width), ctypes.byref(height),
                             ctypes.byref(dt))
  if not app.ntv_state:
    err('cannot initialise application\n')
    return 1
  app.width=max(1, width.value)
  app.height=max(1, height.value)
  app.dt=dt.value
  app.must_quit=False
  #
  make_gui(app)
  app.top.deiconify()
  app.top.mainloop()
  return 0

if __name__=='__main__':
  sys.exit(main(sys.argv))

#~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
