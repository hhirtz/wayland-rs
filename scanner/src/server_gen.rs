use std::borrow::Cow;
use std::io::Write;

use protocol::*;
use common_gen::*;

pub fn generate_server_api<O: Write>(protocol: Protocol, out: &mut O) {
    writeln!(out, "//\n// This file was auto-generated, do not edit directly\n//\n").unwrap();

    if let Some(ref text) = protocol.copyright {
        writeln!(out, "/*\n{}\n*/\n", text).unwrap();
    }

    writeln!(out, "use wayland_sys::common::*;").unwrap();
    writeln!(out, "use wayland_sys::server::*;").unwrap();
    writeln!(out, "use {{Resource, ResourceId, wrap_resource}};").unwrap();
    writeln!(out, "use client::{{ClientId, wrap_client}};").unwrap();
    writeln!(out, "use requests::{{RequestIterator, RequestFifo, get_requestiter_internals}};").unwrap();
    writeln!(out, "use super::interfaces::*;").unwrap();
    writeln!(out, "").unwrap();
    writeln!(out, "use std::ffi::{{CString, CStr}};").unwrap();
    writeln!(out, "use std::ptr;").unwrap();
    writeln!(out, "use std::sync::Arc;").unwrap();
    writeln!(out, "use std::sync::atomic::{{AtomicBool, Ordering}};").unwrap();
    writeln!(out, "use std::os::raw::{{c_void, c_char}};").unwrap();

    emit_request_machinery(&protocol, out);

    let mut bitfields = Vec::new();

    for interface in &protocol.interfaces {
        bitfields.extend(emit_enums(&interface.enums, &interface.name, out));
    }

    for interface in protocol.interfaces {
        if &interface.name[..] == "wl_display" ||
           &interface.name[..] == "wl_registry" {
            // these two are handled by the lib, don't do them.
            continue;
        }
        let camel_iname = snake_to_camel(&interface.name);
        writeln!(out, "//\n// interface {}\n//\n", interface.name).unwrap();

        emit_iface_struct(&camel_iname, &interface, &protocol.name, out);

        emit_opcodes(&interface.name, &interface.events, out);

        if interface.requests.len() > 0 {
            emit_message_enums(&interface.requests, &interface.name, &camel_iname, &bitfields, true, out);
            emit_message_handlers(&interface.requests, &interface.name, &camel_iname, &protocol.name, out);
        }
        emit_iface_impl(&interface.events, &interface.name, &camel_iname, &bitfields, out);
    }
    
}

fn emit_request_machinery<O: Write>(protocol: &Protocol, out: &mut O) {
    writeln!(out, "/// A request generated by the protocol {}.", protocol.name).unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Each variant is composed of a `ClientId` reffering to the client object,").unwrap();
    writeln!(out, "/// a `ResourceId` reffering to the resource object and the event data itself.").unwrap();
    writeln!(out, "#[derive(Debug)]").unwrap();
    writeln!(out, "pub enum {}ProtocolRequest {{", snake_to_camel(&protocol.name)).unwrap();
    for interface in &protocol.interfaces {
        if &interface.name[..] == "wl_display" ||
           &interface.name[..] == "wl_registry" {
            // these two are handled by the lib, don't do them.
            continue;
        }
        if interface.requests.len() > 0 {
            writeln!(out, "    {}(ClientId, ResourceId, {}Request),",
                snake_to_camel(&interface.name), snake_to_camel(&interface.name)).unwrap();
        }
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "type {}_dispatcher_implem = fn(*mut wl_resource, u32, *const wl_argument) -> Option<{}ProtocolRequest>;\n",
        protocol.name, snake_to_camel(&protocol.name)).unwrap();

    writeln!(out,
        "extern \"C\" fn event_dispatcher(implem: *const c_void, resource: *mut c_void, opcode: u32, _: *const wl_message, args: *const wl_argument) {{").unwrap();
    writeln!(out, "    let userdata = unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_user_data, resource as *mut wl_resource) }} as *const (RequestFifo, AtomicBool);").unwrap();
    writeln!(out, "    if userdata.is_null() {{ return; }}").unwrap();
    writeln!(out, "    let fifo: &(RequestFifo, AtomicBool) = unsafe {{ &*userdata }};").unwrap();
    writeln!(out, "    if !fifo.1.load(Ordering::SeqCst) {{ return; }}").unwrap();
    writeln!(out, "    let implem = unsafe {{ ::std::mem::transmute::<_, {}_dispatcher_implem>(implem) }};",
        protocol.name).unwrap();
    writeln!(out, "    let request = implem(resource as *mut wl_resource, opcode, args);").unwrap();
    writeln!(out, "    if let Some(req) = request {{").unwrap();
    writeln!(out, "        fifo.0.push(::Request::{}(req));", snake_to_camel(&protocol.name)).unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

fn emit_iface_struct<O: Write>(camel_iname: &str, interface: &Interface, pname: &str, out: &mut O) {
    if let Some((ref summary, ref desc)) = interface.description {
        write_doc(summary, desc, "", out)
    }

    writeln!(out, "pub struct {} {{\n    ptr: *mut wl_resource,\n    client: *mut wl_client,", camel_iname).unwrap();
    writeln!(out, "    evq: Arc<(RequestFifo,AtomicBool)>\n}}\n").unwrap();

    writeln!(out, "unsafe impl Sync for {} {{}}", camel_iname).unwrap();
    writeln!(out, "unsafe impl Send for {} {{}}", camel_iname).unwrap();

    writeln!(out, "impl Resource for {} {{", camel_iname).unwrap();
    writeln!(out, "    fn ptr(&self) -> *mut wl_resource {{ self.ptr }}").unwrap();
    writeln!(out, "    fn interface() -> *mut wl_interface {{ unsafe {{ &mut {}_interface  as *mut wl_interface }} }}", interface.name).unwrap();
    writeln!(out, "    fn interface_name() -> &'static str {{ \"{}\" }}", interface.name).unwrap();
    writeln!(out, "    fn max_version() -> u32 {{ {} }}", interface.version).unwrap();
    writeln!(out, "    fn bound_version(&self) -> u32 {{ let v = unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_version, self.ptr) }}; v as u32 }}").unwrap();
    writeln!(out, "    fn id(&self) -> ResourceId {{ ResourceId {{ id: self.ptr as usize }} }}").unwrap();
    writeln!(out, "    fn client_id(&self) -> ClientId {{ wrap_client(self.client) }}").unwrap();
    writeln!(out, "    unsafe fn from_ptr(ptr: *mut wl_resource) -> {} {{", camel_iname).unwrap();
    if interface.requests.len() > 0 {
        writeln!(out, "        ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_set_dispatcher, ptr, event_dispatcher, {}_implem as *const c_void, ptr::null_mut(), ::std::mem::transmute::<*const u8, wl_resource_destroy_func_t>(ptr::null()));", interface.name).unwrap();
    }
    writeln!(out, "        let client = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_client, ptr);").unwrap();
    writeln!(out, "        {} {{ ptr: ptr, client: client, evq: Arc::new((RequestFifo::new(), AtomicBool::new(false))) }}", camel_iname).unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    unsafe fn from_ptr_no_own(ptr: *mut wl_resource) -> {} {{", camel_iname).unwrap();
    writeln!(out, "        let client = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_client, ptr);").unwrap();
    writeln!(out, "        {} {{ ptr: ptr, client: client, evq: Arc::new((RequestFifo::new(), AtomicBool::new(false))) }}", camel_iname).unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "    fn set_req_iterator(&mut self, req: &RequestIterator) {{").unwrap();
    writeln!(out, "        self.evq = get_requestiter_internals(req);").unwrap();
    writeln!(out, "        let ptr = &*self.evq as *const (RequestFifo,AtomicBool);").unwrap();
    writeln!(out, "        unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_set_user_data, self.ptr, ptr as *const c_void as *mut c_void) }};").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl ::std::fmt::Debug for {} {{", camel_iname).unwrap();
    writeln!(out, "    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {{").unwrap();
    writeln!(out, "        fmt.write_fmt(format_args!(\"{}::{}::{{:p}}@{{:p}}\", self.ptr, self.client))", pname, interface.name).unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
}

fn emit_message_handlers<O: Write>(messages: &[Message], iname: &str, camel_iname: &str, pname: &str, out: &mut O) {
    writeln!(out, "fn {}_implem(resource: *mut wl_resource, opcode: u32, args: *const wl_argument) -> Option<{}ProtocolRequest> {{",
        iname, snake_to_camel(pname)).unwrap();
    writeln!(out, "    let event = match opcode {{").unwrap();
    for (op, evt) in messages.iter().enumerate() {
        writeln!(out, "        {} => {{", op).unwrap();
        for (i, arg) in evt.args.iter().enumerate() {
            write!(out, "            let arg_{} = unsafe {{", i).unwrap();
            match arg.typ {
                Type::Uint => write!(out, "*(args.offset({}) as *const u32)", i),
                Type::Int | Type::Fd => write!(out, "*(args.offset({}) as *const i32)", i),
                Type::Fixed => write!(out, "wl_fixed_to_double(*(args.offset({}) as *const i32))", i),
                Type::Object => write!(out, "wrap_resource(*(args.offset({}) as *const *mut wl_resource))", i),
                Type::String => write!(out, "String::from_utf8_lossy(CStr::from_ptr(*(args.offset({}) as *const *mut c_char)).to_bytes()).into_owned()", i),
                Type::Array => write!(out, "{{ let array = *(args.offset({}) as *const *mut wl_array); ::std::slice::from_raw_parts((*array).data as *const u8, (*array).size as usize).to_owned() }}", i),
                Type::NewId => { write!(out, "{{").unwrap();
                    write!(out, "let id = *(args.offset({}) as *const u32);", i).unwrap();
                    write!(out, "let client = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_client, resource);").unwrap();
                    write!(out, "let version = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_version, resource);").unwrap();
                    write!(out, "let ptr = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_create, client, &{}_interface, version, id);", arg.interface.as_ref().unwrap()).unwrap();
                    write!(out, "let udata = ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_user_data, resource);").unwrap();
                    write!(out, "let obj = {}::from_ptr(ptr);", snake_to_camel(arg.interface.as_ref().unwrap())).unwrap();
                    write!(out, "ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_set_user_data, obj.ptr(), udata);").unwrap();
                    write!(out, "obj }}")
                },
                Type::Destructor => unreachable!()
            }.unwrap();
            writeln!(out, "}};").unwrap();
            if let Some(ref enu) = arg.enum_ {
                let mut it = enu.split('.');
                match (it.next(), it.next()) {
                    (Some(other_iname), Some(enu_name)) => {
                        write!(out, "            let arg_{} = match {}_{}_from_raw(arg_{} as u32) {{",
                            i, other_iname, enu_name, i).unwrap();
                    }
                    _ => {
                        write!(out, "            let arg_{} = match {}_{}_from_raw(arg_{} as u32) {{",
                            i, iname, enu, i).unwrap();
                    }
                }
                write!(out, " Some(a) => a,").unwrap();
                write!(out, " None => return None").unwrap();
                writeln!(out, " }};").unwrap();
            }
        }
        write!(out, "            Some({}Request::{}", camel_iname, snake_to_camel(&evt.name)).unwrap();
        if evt.args.len() > 0 {
            write!(out, "(").unwrap();
            for i in 0..evt.args.len() {
                write!(out, "arg_{},", i).unwrap();
            }
            write!(out, ")").unwrap();
        }
        writeln!(out, ")").unwrap();
        writeln!(out, "        }},").unwrap();
    }

    writeln!(out, "        _ => None").unwrap();
    writeln!(out, "    }};").unwrap();
    writeln!(out, "    let client = unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_get_client, resource) }};").unwrap();
    writeln!(out, "    event.map(|event| {}ProtocolRequest::{}(wrap_client(client), wrap_resource(resource), event))", snake_to_camel(pname), camel_iname).unwrap();
    writeln!(out, "}}\n").unwrap();
}

fn emit_iface_impl<O: Write>(events: &[Message], iname: &str, camel_iname: &str, bitfields: &[String], out: &mut O) {

    writeln!(out, "impl {} {{", camel_iname).unwrap();
    // requests
    for evt in events {

        if let Some((ref summary, ref doc)) = evt.description {
            write_doc(summary, doc, "    ", out);
        }
        if evt.since > 1 {
            writeln!(out, "    ///\n    /// Requires interface version `>= {}`.", evt.since).unwrap();
        }

        write!(out, "    pub  fn send_{}", evt.name).unwrap();
        write!(out, "(&self,").unwrap();
        for a in &evt.args {
            if a.typ == Type::NewId {
                write!(out, " {}: &{},", a.name, snake_to_camel(a.interface.as_ref().unwrap())).unwrap();
                continue;
            }
            if let Some(ref enu) = a.enum_ {
                write!(out, " {}: ", a.name).unwrap();
                if enu.contains('.') {
                    if bitfields.contains(enu) {
                        write!(out, "{}::", dotted_snake_to_camel(enu)).unwrap();
                    }
                    write!(out, "{},", dotted_snake_to_camel(enu)).unwrap();
                } else {
                    if bitfields.contains(&format!("{}.{}", iname, enu)) {
                        write!(out, "{}{}::", camel_iname, snake_to_camel(enu)).unwrap();
                    }
                    write!(out, "{}{},", camel_iname, snake_to_camel(enu)).unwrap();
                }
                continue;
            }
            let typ: Cow<str> = if a.typ == Type::Object {
                a.interface.as_ref().map(|i| format!("&{}", snake_to_camel(i)).into()).unwrap_or("*mut ()".into())
            } else {
                a.typ.rust_type().into()
            };
            if a.allow_null {
                write!(out, " {}: Option<{}>,", a.name, typ).unwrap();
            } else {
                write!(out, " {}: {},", a.name, typ).unwrap();
            }
        }
        write!(out, ")").unwrap();
        writeln!(out, " {{").unwrap();
        // function body
        for a in &evt.args {
            match a.typ {
                Type::String => {
                    if a.allow_null {
                        writeln!(out, "        let {} = {}.map(|t| CString::new(t).unwrap_or_else(|_| panic!(\"Got a String with interior null.\")));",
                            a.name, a.name).unwrap();
                    } else {
                        writeln!(out, "        let {} = CString::new({}).unwrap_or_else(|_| panic!(\"Got a String with interior null.\"));",
                            a.name, a.name).unwrap();
                    }
                },
                Type::Fixed => {
                    writeln!(out, "        let {} = wl_fixed_from_double({});", a.name, a.name).unwrap();
                },
                _ => {}
            }
        }
        for a in &evt.args {
            if a.typ == Type::Array {
                if a.allow_null {
                    write!(out, "let {} = {}.as_mut().map(|v| wl_array {{ size: v.len(), alloc: v.capacity(), data: v.as_ptr() as *mut _ }});",
                        a.name, a.name).unwrap();
                } else {
                    write!(out, "let {} = wl_array {{ size: {}.len(), alloc: {}.capacity(), data: {}.as_ptr() as *mut _ }};",
                        a.name, a.name, a.name, a.name).unwrap();
                }
            }
        }
        writeln!(out, "        unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_post_event, self.ptr(), {}_{}",
            snake_to_screaming(iname), snake_to_screaming(&evt.name)).unwrap();
        write!(out, "           ").unwrap();
        for a in &evt.args {
            if let Some(ref enu) = a.enum_ {
                write!(out, ", {}", a.name).unwrap();
                if bitfields.contains(enu) || bitfields.contains(&format!("{}.{}", iname, enu)) {
                    write!(out, ".bits()").unwrap();
                }
                write!(out, " as {}", if a.typ == Type::Uint { "u32" } else { "i32" }).unwrap();
            } else if a.typ == Type::String {
                if a.allow_null {
                    write!(out, ", {}.map(|s| s.as_ptr()).unwrap_or(ptr::null())", a.name).unwrap();
                } else {
                    write!(out, ", {}.as_ptr()", a.name).unwrap();
                }
            } else if a.typ == Type::Array {
                if a.allow_null {
                    write!(out, ", {}.map(|a| &a as *const wl_array).unwrap_or(ptr::null_mut())", a.name).unwrap();
                } else {
                    write!(out, ", &{} as *const wl_array", a.name).unwrap();
                }
            } else if a.typ == Type::Object || a.typ == Type::NewId {
                if a.allow_null {
                    write!(out, ", {}.map(Resource::ptr).unwrap_or(ptr::null_mut())", a.name).unwrap();
                } else {
                    write!(out, ", {}.ptr()", a.name).unwrap();
                }
            } else {
                if a.allow_null {
                    write!(out, ", {}.unwrap_or(0)", a.name).unwrap();
                } else {
                    write!(out, ", {}", a.name).unwrap();
                }
            }
        }
        writeln!(out, ") }};").unwrap();
        writeln!(out, "    }}").unwrap();
    }

    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Drop for {} {{", camel_iname).unwrap();
    writeln!(out, "    fn drop(&mut self) {{").unwrap();
    writeln!(out, "        unsafe {{ ffi_dispatch!(WAYLAND_SERVER_HANDLE, wl_resource_destroy, self.ptr()) }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}