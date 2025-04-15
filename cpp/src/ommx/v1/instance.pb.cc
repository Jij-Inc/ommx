// Generated by the protocol buffer compiler.  DO NOT EDIT!
// source: ommx/v1/instance.proto
// Protobuf C++ Version: 5.26.1

#include "ommx/v1/instance.pb.h"

#include <algorithm>
#include "google/protobuf/io/coded_stream.h"
#include "google/protobuf/extension_set.h"
#include "google/protobuf/wire_format_lite.h"
#include "google/protobuf/descriptor.h"
#include "google/protobuf/generated_message_reflection.h"
#include "google/protobuf/reflection_ops.h"
#include "google/protobuf/wire_format.h"
#include "google/protobuf/generated_message_tctable_impl.h"
// @@protoc_insertion_point(includes)

// Must be included last.
#include "google/protobuf/port_def.inc"
PROTOBUF_PRAGMA_INIT_SEG
namespace _pb = ::google::protobuf;
namespace _pbi = ::google::protobuf::internal;
namespace _fl = ::google::protobuf::internal::field_layout;
namespace ommx {
namespace v1 {

inline constexpr Instance_Description::Impl_::Impl_(
    ::_pbi::ConstantInitialized) noexcept
      : _cached_size_{0},
        authors_{},
        name_(
            &::google::protobuf::internal::fixed_address_empty_string,
            ::_pbi::ConstantInitialized()),
        description_(
            &::google::protobuf::internal::fixed_address_empty_string,
            ::_pbi::ConstantInitialized()),
        created_by_(
            &::google::protobuf::internal::fixed_address_empty_string,
            ::_pbi::ConstantInitialized()) {}

template <typename>
PROTOBUF_CONSTEXPR Instance_Description::Instance_Description(::_pbi::ConstantInitialized)
    : _impl_(::_pbi::ConstantInitialized()) {}
struct Instance_DescriptionDefaultTypeInternal {
  PROTOBUF_CONSTEXPR Instance_DescriptionDefaultTypeInternal() : _instance(::_pbi::ConstantInitialized{}) {}
  ~Instance_DescriptionDefaultTypeInternal() {}
  union {
    Instance_Description _instance;
  };
};

PROTOBUF_ATTRIBUTE_NO_DESTROY PROTOBUF_CONSTINIT
    PROTOBUF_ATTRIBUTE_INIT_PRIORITY1 Instance_DescriptionDefaultTypeInternal _Instance_Description_default_instance_;

inline constexpr Instance::Impl_::Impl_(
    ::_pbi::ConstantInitialized) noexcept
      : _cached_size_{0},
        decision_variables_{},
        constraints_{},
        description_{nullptr},
        objective_{nullptr},
        sense_{static_cast< ::ommx::v1::Instance_Sense >(0)} {}

template <typename>
PROTOBUF_CONSTEXPR Instance::Instance(::_pbi::ConstantInitialized)
    : _impl_(::_pbi::ConstantInitialized()) {}
struct InstanceDefaultTypeInternal {
  PROTOBUF_CONSTEXPR InstanceDefaultTypeInternal() : _instance(::_pbi::ConstantInitialized{}) {}
  ~InstanceDefaultTypeInternal() {}
  union {
    Instance _instance;
  };
};

PROTOBUF_ATTRIBUTE_NO_DESTROY PROTOBUF_CONSTINIT
    PROTOBUF_ATTRIBUTE_INIT_PRIORITY1 InstanceDefaultTypeInternal _Instance_default_instance_;
}  // namespace v1
}  // namespace ommx
static ::_pb::Metadata file_level_metadata_ommx_2fv1_2finstance_2eproto[2];
static const ::_pb::EnumDescriptor* file_level_enum_descriptors_ommx_2fv1_2finstance_2eproto[1];
static constexpr const ::_pb::ServiceDescriptor**
    file_level_service_descriptors_ommx_2fv1_2finstance_2eproto = nullptr;
const ::uint32_t
    TableStruct_ommx_2fv1_2finstance_2eproto::offsets[] ABSL_ATTRIBUTE_SECTION_VARIABLE(
        protodesc_cold) = {
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _impl_._has_bits_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _internal_metadata_),
        ~0u,  // no _extensions_
        ~0u,  // no _oneof_case_
        ~0u,  // no _weak_field_map_
        ~0u,  // no _inlined_string_donated_
        ~0u,  // no _split_
        ~0u,  // no sizeof(Split)
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _impl_.name_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _impl_.description_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _impl_.authors_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance_Description, _impl_.created_by_),
        0,
        1,
        ~0u,
        2,
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_._has_bits_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _internal_metadata_),
        ~0u,  // no _extensions_
        ~0u,  // no _oneof_case_
        ~0u,  // no _weak_field_map_
        ~0u,  // no _inlined_string_donated_
        ~0u,  // no _split_
        ~0u,  // no sizeof(Split)
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_.description_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_.decision_variables_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_.objective_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_.constraints_),
        PROTOBUF_FIELD_OFFSET(::ommx::v1::Instance, _impl_.sense_),
        0,
        ~0u,
        1,
        ~0u,
        ~0u,
};

static const ::_pbi::MigrationSchema
    schemas[] ABSL_ATTRIBUTE_SECTION_VARIABLE(protodesc_cold) = {
        {0, 12, -1, sizeof(::ommx::v1::Instance_Description)},
        {16, 29, -1, sizeof(::ommx::v1::Instance)},
};
static const ::_pb::Message* const file_default_instances[] = {
    &::ommx::v1::_Instance_Description_default_instance_._instance,
    &::ommx::v1::_Instance_default_instance_._instance,
};
const char descriptor_table_protodef_ommx_2fv1_2finstance_2eproto[] ABSL_ATTRIBUTE_SECTION_VARIABLE(
    protodesc_cold) = {
    "\n\026ommx/v1/instance.proto\022\007ommx.v1\032\030ommx/"
    "v1/constraint.proto\032 ommx/v1/decision_va"
    "riables.proto\032\026ommx/v1/function.proto\"\252\004"
    "\n\010Instance\022\?\n\013description\030\001 \001(\0132\035.ommx.v"
    "1.Instance.DescriptionR\013description\022H\n\022d"
    "ecision_variables\030\002 \003(\0132\031.ommx.v1.Decisi"
    "onVariableR\021decisionVariables\022/\n\tobjecti"
    "ve\030\003 \001(\0132\021.ommx.v1.FunctionR\tobjective\0225"
    "\n\013constraints\030\004 \003(\0132\023.ommx.v1.Constraint"
    "R\013constraints\022-\n\005sense\030\005 \001(\0162\027.ommx.v1.I"
    "nstance.SenseR\005sense\032\263\001\n\013Description\022\027\n\004"
    "name\030\001 \001(\tH\000R\004name\210\001\001\022%\n\013description\030\002 \001"
    "(\tH\001R\013description\210\001\001\022\030\n\007authors\030\003 \003(\tR\007a"
    "uthors\022\"\n\ncreated_by\030\004 \001(\tH\002R\tcreatedBy\210"
    "\001\001B\007\n\005_nameB\016\n\014_descriptionB\r\n\013_created_"
    "by\"F\n\005Sense\022\025\n\021SENSE_UNSPECIFIED\020\000\022\022\n\016SE"
    "NSE_MINIMIZE\020\001\022\022\n\016SENSE_MAXIMIZE\020\002BY\n\013co"
    "m.ommx.v1B\rInstanceProtoP\001\242\002\003OXX\252\002\007Ommx."
    "V1\312\002\007Ommx\\V1\342\002\023Ommx\\V1\\GPBMetadata\352\002\010Omm"
    "x::V1b\006proto3"
};
static const ::_pbi::DescriptorTable* const descriptor_table_ommx_2fv1_2finstance_2eproto_deps[3] =
    {
        &::descriptor_table_ommx_2fv1_2fconstraint_2eproto,
        &::descriptor_table_ommx_2fv1_2fdecision_5fvariables_2eproto,
        &::descriptor_table_ommx_2fv1_2ffunction_2eproto,
};
static ::absl::once_flag descriptor_table_ommx_2fv1_2finstance_2eproto_once;
const ::_pbi::DescriptorTable descriptor_table_ommx_2fv1_2finstance_2eproto = {
    false,
    false,
    773,
    descriptor_table_protodef_ommx_2fv1_2finstance_2eproto,
    "ommx/v1/instance.proto",
    &descriptor_table_ommx_2fv1_2finstance_2eproto_once,
    descriptor_table_ommx_2fv1_2finstance_2eproto_deps,
    3,
    2,
    schemas,
    file_default_instances,
    TableStruct_ommx_2fv1_2finstance_2eproto::offsets,
    file_level_metadata_ommx_2fv1_2finstance_2eproto,
    file_level_enum_descriptors_ommx_2fv1_2finstance_2eproto,
    file_level_service_descriptors_ommx_2fv1_2finstance_2eproto,
};

// This function exists to be marked as weak.
// It can significantly speed up compilation by breaking up LLVM's SCC
// in the .pb.cc translation units. Large translation units see a
// reduction of more than 35% of walltime for optimized builds. Without
// the weak attribute all the messages in the file, including all the
// vtables and everything they use become part of the same SCC through
// a cycle like:
// GetMetadata -> descriptor table -> default instances ->
//   vtables -> GetMetadata
// By adding a weak function here we break the connection from the
// individual vtables back into the descriptor table.
PROTOBUF_ATTRIBUTE_WEAK const ::_pbi::DescriptorTable* descriptor_table_ommx_2fv1_2finstance_2eproto_getter() {
  return &descriptor_table_ommx_2fv1_2finstance_2eproto;
}
namespace ommx {
namespace v1 {
const ::google::protobuf::EnumDescriptor* Instance_Sense_descriptor() {
  ::google::protobuf::internal::AssignDescriptors(&descriptor_table_ommx_2fv1_2finstance_2eproto);
  return file_level_enum_descriptors_ommx_2fv1_2finstance_2eproto[0];
}
PROTOBUF_CONSTINIT const uint32_t Instance_Sense_internal_data_[] = {
    196608u, 0u, };
bool Instance_Sense_IsValid(int value) {
  return 0 <= value && value <= 2;
}
#if (__cplusplus < 201703) && \
  (!defined(_MSC_VER) || (_MSC_VER >= 1900 && _MSC_VER < 1912))

constexpr Instance_Sense Instance::SENSE_UNSPECIFIED;
constexpr Instance_Sense Instance::SENSE_MINIMIZE;
constexpr Instance_Sense Instance::SENSE_MAXIMIZE;
constexpr Instance_Sense Instance::Sense_MIN;
constexpr Instance_Sense Instance::Sense_MAX;
constexpr int Instance::Sense_ARRAYSIZE;

#endif  // (__cplusplus < 201703) &&
        // (!defined(_MSC_VER) || (_MSC_VER >= 1900 && _MSC_VER < 1912))
// ===================================================================

class Instance_Description::_Internal {
 public:
  using HasBits = decltype(std::declval<Instance_Description>()._impl_._has_bits_);
  static constexpr ::int32_t kHasBitsOffset =
    8 * PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_._has_bits_);
};

Instance_Description::Instance_Description(::google::protobuf::Arena* arena)
    : ::google::protobuf::Message(arena) {
  SharedCtor(arena);
  // @@protoc_insertion_point(arena_constructor:ommx.v1.Instance.Description)
}
inline PROTOBUF_NDEBUG_INLINE Instance_Description::Impl_::Impl_(
    ::google::protobuf::internal::InternalVisibility visibility, ::google::protobuf::Arena* arena,
    const Impl_& from)
      : _has_bits_{from._has_bits_},
        _cached_size_{0},
        authors_{visibility, arena, from.authors_},
        name_(arena, from.name_),
        description_(arena, from.description_),
        created_by_(arena, from.created_by_) {}

Instance_Description::Instance_Description(
    ::google::protobuf::Arena* arena,
    const Instance_Description& from)
    : ::google::protobuf::Message(arena) {
  Instance_Description* const _this = this;
  (void)_this;
  _internal_metadata_.MergeFrom<::google::protobuf::UnknownFieldSet>(
      from._internal_metadata_);
  new (&_impl_) Impl_(internal_visibility(), arena, from._impl_);

  // @@protoc_insertion_point(copy_constructor:ommx.v1.Instance.Description)
}
inline PROTOBUF_NDEBUG_INLINE Instance_Description::Impl_::Impl_(
    ::google::protobuf::internal::InternalVisibility visibility,
    ::google::protobuf::Arena* arena)
      : _cached_size_{0},
        authors_{visibility, arena},
        name_(arena),
        description_(arena),
        created_by_(arena) {}

inline void Instance_Description::SharedCtor(::_pb::Arena* arena) {
  new (&_impl_) Impl_(internal_visibility(), arena);
}
Instance_Description::~Instance_Description() {
  // @@protoc_insertion_point(destructor:ommx.v1.Instance.Description)
  _internal_metadata_.Delete<::google::protobuf::UnknownFieldSet>();
  SharedDtor();
}
inline void Instance_Description::SharedDtor() {
  ABSL_DCHECK(GetArena() == nullptr);
  _impl_.name_.Destroy();
  _impl_.description_.Destroy();
  _impl_.created_by_.Destroy();
  _impl_.~Impl_();
}

const ::google::protobuf::MessageLite::ClassData*
Instance_Description::GetClassData() const {
  PROTOBUF_CONSTINIT static const ::google::protobuf::MessageLite::
      ClassDataFull _data_ = {
          {
              nullptr,  // OnDemandRegisterArenaDtor
              PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_._cached_size_),
              false,
          },
          &Instance_Description::MergeImpl,
          &Instance_Description::kDescriptorMethods,
      };
  return &_data_;
}
PROTOBUF_NOINLINE void Instance_Description::Clear() {
// @@protoc_insertion_point(message_clear_start:ommx.v1.Instance.Description)
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  ::uint32_t cached_has_bits = 0;
  // Prevent compiler warnings about cached_has_bits being unused
  (void) cached_has_bits;

  _impl_.authors_.Clear();
  cached_has_bits = _impl_._has_bits_[0];
  if (cached_has_bits & 0x00000007u) {
    if (cached_has_bits & 0x00000001u) {
      _impl_.name_.ClearNonDefaultToEmpty();
    }
    if (cached_has_bits & 0x00000002u) {
      _impl_.description_.ClearNonDefaultToEmpty();
    }
    if (cached_has_bits & 0x00000004u) {
      _impl_.created_by_.ClearNonDefaultToEmpty();
    }
  }
  _impl_._has_bits_.Clear();
  _internal_metadata_.Clear<::google::protobuf::UnknownFieldSet>();
}

const char* Instance_Description::_InternalParse(
    const char* ptr, ::_pbi::ParseContext* ctx) {
  ptr = ::_pbi::TcParser::ParseLoop(this, ptr, ctx, &_table_.header);
  return ptr;
}


PROTOBUF_CONSTINIT PROTOBUF_ATTRIBUTE_INIT_PRIORITY1
const ::_pbi::TcParseTable<2, 4, 0, 69, 2> Instance_Description::_table_ = {
  {
    PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_._has_bits_),
    0, // no _extensions_
    4, 24,  // max_field_number, fast_idx_mask
    offsetof(decltype(_table_), field_lookup_table),
    4294967280,  // skipmap
    offsetof(decltype(_table_), field_entries),
    4,  // num_field_entries
    0,  // num_aux_entries
    offsetof(decltype(_table_), field_names),  // no aux_entries
    &_Instance_Description_default_instance_._instance,
    ::_pbi::TcParser::GenericFallback,  // fallback
    #ifdef PROTOBUF_PREFETCH_PARSE_TABLE
    ::_pbi::TcParser::GetTable<::ommx::v1::Instance_Description>(),  // to_prefetch
    #endif  // PROTOBUF_PREFETCH_PARSE_TABLE
  }, {{
    // optional string created_by = 4 [json_name = "createdBy"];
    {::_pbi::TcParser::FastUS1,
     {34, 2, 0, PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.created_by_)}},
    // optional string name = 1 [json_name = "name"];
    {::_pbi::TcParser::FastUS1,
     {10, 0, 0, PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.name_)}},
    // optional string description = 2 [json_name = "description"];
    {::_pbi::TcParser::FastUS1,
     {18, 1, 0, PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.description_)}},
    // repeated string authors = 3 [json_name = "authors"];
    {::_pbi::TcParser::FastUR1,
     {26, 63, 0, PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.authors_)}},
  }}, {{
    65535, 65535
  }}, {{
    // optional string name = 1 [json_name = "name"];
    {PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.name_), _Internal::kHasBitsOffset + 0, 0,
    (0 | ::_fl::kFcOptional | ::_fl::kUtf8String | ::_fl::kRepAString)},
    // optional string description = 2 [json_name = "description"];
    {PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.description_), _Internal::kHasBitsOffset + 1, 0,
    (0 | ::_fl::kFcOptional | ::_fl::kUtf8String | ::_fl::kRepAString)},
    // repeated string authors = 3 [json_name = "authors"];
    {PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.authors_), -1, 0,
    (0 | ::_fl::kFcRepeated | ::_fl::kUtf8String | ::_fl::kRepSString)},
    // optional string created_by = 4 [json_name = "createdBy"];
    {PROTOBUF_FIELD_OFFSET(Instance_Description, _impl_.created_by_), _Internal::kHasBitsOffset + 2, 0,
    (0 | ::_fl::kFcOptional | ::_fl::kUtf8String | ::_fl::kRepAString)},
  }},
  // no aux_entries
  {{
    "\34\4\13\7\12\0\0\0"
    "ommx.v1.Instance.Description"
    "name"
    "description"
    "authors"
    "created_by"
  }},
};

::uint8_t* Instance_Description::_InternalSerialize(
    ::uint8_t* target,
    ::google::protobuf::io::EpsCopyOutputStream* stream) const {
  // @@protoc_insertion_point(serialize_to_array_start:ommx.v1.Instance.Description)
  ::uint32_t cached_has_bits = 0;
  (void)cached_has_bits;

  cached_has_bits = _impl_._has_bits_[0];
  // optional string name = 1 [json_name = "name"];
  if (cached_has_bits & 0x00000001u) {
    const std::string& _s = this->_internal_name();
    ::google::protobuf::internal::WireFormatLite::VerifyUtf8String(
        _s.data(), static_cast<int>(_s.length()), ::google::protobuf::internal::WireFormatLite::SERIALIZE, "ommx.v1.Instance.Description.name");
    target = stream->WriteStringMaybeAliased(1, _s, target);
  }

  // optional string description = 2 [json_name = "description"];
  if (cached_has_bits & 0x00000002u) {
    const std::string& _s = this->_internal_description();
    ::google::protobuf::internal::WireFormatLite::VerifyUtf8String(
        _s.data(), static_cast<int>(_s.length()), ::google::protobuf::internal::WireFormatLite::SERIALIZE, "ommx.v1.Instance.Description.description");
    target = stream->WriteStringMaybeAliased(2, _s, target);
  }

  // repeated string authors = 3 [json_name = "authors"];
  for (int i = 0, n = this->_internal_authors_size(); i < n; ++i) {
    const auto& s = this->_internal_authors().Get(i);
    ::google::protobuf::internal::WireFormatLite::VerifyUtf8String(
        s.data(), static_cast<int>(s.length()), ::google::protobuf::internal::WireFormatLite::SERIALIZE, "ommx.v1.Instance.Description.authors");
    target = stream->WriteString(3, s, target);
  }

  // optional string created_by = 4 [json_name = "createdBy"];
  if (cached_has_bits & 0x00000004u) {
    const std::string& _s = this->_internal_created_by();
    ::google::protobuf::internal::WireFormatLite::VerifyUtf8String(
        _s.data(), static_cast<int>(_s.length()), ::google::protobuf::internal::WireFormatLite::SERIALIZE, "ommx.v1.Instance.Description.created_by");
    target = stream->WriteStringMaybeAliased(4, _s, target);
  }

  if (PROTOBUF_PREDICT_FALSE(_internal_metadata_.have_unknown_fields())) {
    target =
        ::_pbi::WireFormat::InternalSerializeUnknownFieldsToArray(
            _internal_metadata_.unknown_fields<::google::protobuf::UnknownFieldSet>(::google::protobuf::UnknownFieldSet::default_instance), target, stream);
  }
  // @@protoc_insertion_point(serialize_to_array_end:ommx.v1.Instance.Description)
  return target;
}

::size_t Instance_Description::ByteSizeLong() const {
// @@protoc_insertion_point(message_byte_size_start:ommx.v1.Instance.Description)
  ::size_t total_size = 0;

  ::uint32_t cached_has_bits = 0;
  // Prevent compiler warnings about cached_has_bits being unused
  (void) cached_has_bits;

  // repeated string authors = 3 [json_name = "authors"];
  total_size += 1 * ::google::protobuf::internal::FromIntSize(_internal_authors().size());
  for (int i = 0, n = _internal_authors().size(); i < n; ++i) {
    total_size += ::google::protobuf::internal::WireFormatLite::StringSize(
        _internal_authors().Get(i));
  }
  cached_has_bits = _impl_._has_bits_[0];
  if (cached_has_bits & 0x00000007u) {
    // optional string name = 1 [json_name = "name"];
    if (cached_has_bits & 0x00000001u) {
      total_size += 1 + ::google::protobuf::internal::WireFormatLite::StringSize(
                                      this->_internal_name());
    }

    // optional string description = 2 [json_name = "description"];
    if (cached_has_bits & 0x00000002u) {
      total_size += 1 + ::google::protobuf::internal::WireFormatLite::StringSize(
                                      this->_internal_description());
    }

    // optional string created_by = 4 [json_name = "createdBy"];
    if (cached_has_bits & 0x00000004u) {
      total_size += 1 + ::google::protobuf::internal::WireFormatLite::StringSize(
                                      this->_internal_created_by());
    }

  }
  return MaybeComputeUnknownFieldsSize(total_size, &_impl_._cached_size_);
}


void Instance_Description::MergeImpl(::google::protobuf::MessageLite& to_msg, const ::google::protobuf::MessageLite& from_msg) {
  auto* const _this = static_cast<Instance_Description*>(&to_msg);
  auto& from = static_cast<const Instance_Description&>(from_msg);
  // @@protoc_insertion_point(class_specific_merge_from_start:ommx.v1.Instance.Description)
  ABSL_DCHECK_NE(&from, _this);
  ::uint32_t cached_has_bits = 0;
  (void) cached_has_bits;

  _this->_internal_mutable_authors()->MergeFrom(from._internal_authors());
  cached_has_bits = from._impl_._has_bits_[0];
  if (cached_has_bits & 0x00000007u) {
    if (cached_has_bits & 0x00000001u) {
      _this->_internal_set_name(from._internal_name());
    }
    if (cached_has_bits & 0x00000002u) {
      _this->_internal_set_description(from._internal_description());
    }
    if (cached_has_bits & 0x00000004u) {
      _this->_internal_set_created_by(from._internal_created_by());
    }
  }
  _this->_impl_._has_bits_[0] |= cached_has_bits;
  _this->_internal_metadata_.MergeFrom<::google::protobuf::UnknownFieldSet>(from._internal_metadata_);
}

void Instance_Description::CopyFrom(const Instance_Description& from) {
// @@protoc_insertion_point(class_specific_copy_from_start:ommx.v1.Instance.Description)
  if (&from == this) return;
  Clear();
  MergeFrom(from);
}

PROTOBUF_NOINLINE bool Instance_Description::IsInitialized() const {
  return true;
}

void Instance_Description::InternalSwap(Instance_Description* PROTOBUF_RESTRICT other) {
  using std::swap;
  auto* arena = GetArena();
  ABSL_DCHECK_EQ(arena, other->GetArena());
  _internal_metadata_.InternalSwap(&other->_internal_metadata_);
  swap(_impl_._has_bits_[0], other->_impl_._has_bits_[0]);
  _impl_.authors_.InternalSwap(&other->_impl_.authors_);
  ::_pbi::ArenaStringPtr::InternalSwap(&_impl_.name_, &other->_impl_.name_, arena);
  ::_pbi::ArenaStringPtr::InternalSwap(&_impl_.description_, &other->_impl_.description_, arena);
  ::_pbi::ArenaStringPtr::InternalSwap(&_impl_.created_by_, &other->_impl_.created_by_, arena);
}

::google::protobuf::Metadata Instance_Description::GetMetadata() const {
  return ::_pbi::AssignDescriptors(&descriptor_table_ommx_2fv1_2finstance_2eproto_getter,
                                   &descriptor_table_ommx_2fv1_2finstance_2eproto_once,
                                   file_level_metadata_ommx_2fv1_2finstance_2eproto[0]);
}
// ===================================================================

class Instance::_Internal {
 public:
  using HasBits = decltype(std::declval<Instance>()._impl_._has_bits_);
  static constexpr ::int32_t kHasBitsOffset =
    8 * PROTOBUF_FIELD_OFFSET(Instance, _impl_._has_bits_);
};

void Instance::clear_decision_variables() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _impl_.decision_variables_.Clear();
}
void Instance::clear_objective() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  if (_impl_.objective_ != nullptr) _impl_.objective_->Clear();
  _impl_._has_bits_[0] &= ~0x00000002u;
}
void Instance::clear_constraints() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _impl_.constraints_.Clear();
}
Instance::Instance(::google::protobuf::Arena* arena)
    : ::google::protobuf::Message(arena) {
  SharedCtor(arena);
  // @@protoc_insertion_point(arena_constructor:ommx.v1.Instance)
}
inline PROTOBUF_NDEBUG_INLINE Instance::Impl_::Impl_(
    ::google::protobuf::internal::InternalVisibility visibility, ::google::protobuf::Arena* arena,
    const Impl_& from)
      : _has_bits_{from._has_bits_},
        _cached_size_{0},
        decision_variables_{visibility, arena, from.decision_variables_},
        constraints_{visibility, arena, from.constraints_} {}

Instance::Instance(
    ::google::protobuf::Arena* arena,
    const Instance& from)
    : ::google::protobuf::Message(arena) {
  Instance* const _this = this;
  (void)_this;
  _internal_metadata_.MergeFrom<::google::protobuf::UnknownFieldSet>(
      from._internal_metadata_);
  new (&_impl_) Impl_(internal_visibility(), arena, from._impl_);
  ::uint32_t cached_has_bits = _impl_._has_bits_[0];
  _impl_.description_ = (cached_has_bits & 0x00000001u) ? ::google::protobuf::Message::CopyConstruct<::ommx::v1::Instance_Description>(
                              arena, *from._impl_.description_)
                        : nullptr;
  _impl_.objective_ = (cached_has_bits & 0x00000002u) ? ::google::protobuf::Message::CopyConstruct<::ommx::v1::Function>(
                              arena, *from._impl_.objective_)
                        : nullptr;
  _impl_.sense_ = from._impl_.sense_;

  // @@protoc_insertion_point(copy_constructor:ommx.v1.Instance)
}
inline PROTOBUF_NDEBUG_INLINE Instance::Impl_::Impl_(
    ::google::protobuf::internal::InternalVisibility visibility,
    ::google::protobuf::Arena* arena)
      : _cached_size_{0},
        decision_variables_{visibility, arena},
        constraints_{visibility, arena} {}

inline void Instance::SharedCtor(::_pb::Arena* arena) {
  new (&_impl_) Impl_(internal_visibility(), arena);
  ::memset(reinterpret_cast<char *>(&_impl_) +
               offsetof(Impl_, description_),
           0,
           offsetof(Impl_, sense_) -
               offsetof(Impl_, description_) +
               sizeof(Impl_::sense_));
}
Instance::~Instance() {
  // @@protoc_insertion_point(destructor:ommx.v1.Instance)
  _internal_metadata_.Delete<::google::protobuf::UnknownFieldSet>();
  SharedDtor();
}
inline void Instance::SharedDtor() {
  ABSL_DCHECK(GetArena() == nullptr);
  delete _impl_.description_;
  delete _impl_.objective_;
  _impl_.~Impl_();
}

const ::google::protobuf::MessageLite::ClassData*
Instance::GetClassData() const {
  PROTOBUF_CONSTINIT static const ::google::protobuf::MessageLite::
      ClassDataFull _data_ = {
          {
              nullptr,  // OnDemandRegisterArenaDtor
              PROTOBUF_FIELD_OFFSET(Instance, _impl_._cached_size_),
              false,
          },
          &Instance::MergeImpl,
          &Instance::kDescriptorMethods,
      };
  return &_data_;
}
PROTOBUF_NOINLINE void Instance::Clear() {
// @@protoc_insertion_point(message_clear_start:ommx.v1.Instance)
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  ::uint32_t cached_has_bits = 0;
  // Prevent compiler warnings about cached_has_bits being unused
  (void) cached_has_bits;

  _impl_.decision_variables_.Clear();
  _impl_.constraints_.Clear();
  cached_has_bits = _impl_._has_bits_[0];
  if (cached_has_bits & 0x00000003u) {
    if (cached_has_bits & 0x00000001u) {
      ABSL_DCHECK(_impl_.description_ != nullptr);
      _impl_.description_->Clear();
    }
    if (cached_has_bits & 0x00000002u) {
      ABSL_DCHECK(_impl_.objective_ != nullptr);
      _impl_.objective_->Clear();
    }
  }
  _impl_.sense_ = 0;
  _impl_._has_bits_.Clear();
  _internal_metadata_.Clear<::google::protobuf::UnknownFieldSet>();
}

const char* Instance::_InternalParse(
    const char* ptr, ::_pbi::ParseContext* ctx) {
  ptr = ::_pbi::TcParser::ParseLoop(this, ptr, ctx, &_table_.header);
  return ptr;
}


PROTOBUF_CONSTINIT PROTOBUF_ATTRIBUTE_INIT_PRIORITY1
const ::_pbi::TcParseTable<3, 5, 4, 0, 2> Instance::_table_ = {
  {
    PROTOBUF_FIELD_OFFSET(Instance, _impl_._has_bits_),
    0, // no _extensions_
    5, 56,  // max_field_number, fast_idx_mask
    offsetof(decltype(_table_), field_lookup_table),
    4294967264,  // skipmap
    offsetof(decltype(_table_), field_entries),
    5,  // num_field_entries
    4,  // num_aux_entries
    offsetof(decltype(_table_), aux_entries),
    &_Instance_default_instance_._instance,
    ::_pbi::TcParser::GenericFallback,  // fallback
    #ifdef PROTOBUF_PREFETCH_PARSE_TABLE
    ::_pbi::TcParser::GetTable<::ommx::v1::Instance>(),  // to_prefetch
    #endif  // PROTOBUF_PREFETCH_PARSE_TABLE
  }, {{
    {::_pbi::TcParser::MiniParse, {}},
    // .ommx.v1.Instance.Description description = 1 [json_name = "description"];
    {::_pbi::TcParser::FastMtS1,
     {10, 0, 0, PROTOBUF_FIELD_OFFSET(Instance, _impl_.description_)}},
    // repeated .ommx.v1.DecisionVariable decision_variables = 2 [json_name = "decisionVariables"];
    {::_pbi::TcParser::FastMtR1,
     {18, 63, 1, PROTOBUF_FIELD_OFFSET(Instance, _impl_.decision_variables_)}},
    // .ommx.v1.Function objective = 3 [json_name = "objective"];
    {::_pbi::TcParser::FastMtS1,
     {26, 1, 2, PROTOBUF_FIELD_OFFSET(Instance, _impl_.objective_)}},
    // repeated .ommx.v1.Constraint constraints = 4 [json_name = "constraints"];
    {::_pbi::TcParser::FastMtR1,
     {34, 63, 3, PROTOBUF_FIELD_OFFSET(Instance, _impl_.constraints_)}},
    // .ommx.v1.Instance.Sense sense = 5 [json_name = "sense"];
    {::_pbi::TcParser::SingularVarintNoZag1<::uint32_t, offsetof(Instance, _impl_.sense_), 63>(),
     {40, 63, 0, PROTOBUF_FIELD_OFFSET(Instance, _impl_.sense_)}},
    {::_pbi::TcParser::MiniParse, {}},
    {::_pbi::TcParser::MiniParse, {}},
  }}, {{
    65535, 65535
  }}, {{
    // .ommx.v1.Instance.Description description = 1 [json_name = "description"];
    {PROTOBUF_FIELD_OFFSET(Instance, _impl_.description_), _Internal::kHasBitsOffset + 0, 0,
    (0 | ::_fl::kFcOptional | ::_fl::kMessage | ::_fl::kTvTable)},
    // repeated .ommx.v1.DecisionVariable decision_variables = 2 [json_name = "decisionVariables"];
    {PROTOBUF_FIELD_OFFSET(Instance, _impl_.decision_variables_), -1, 1,
    (0 | ::_fl::kFcRepeated | ::_fl::kMessage | ::_fl::kTvTable)},
    // .ommx.v1.Function objective = 3 [json_name = "objective"];
    {PROTOBUF_FIELD_OFFSET(Instance, _impl_.objective_), _Internal::kHasBitsOffset + 1, 2,
    (0 | ::_fl::kFcOptional | ::_fl::kMessage | ::_fl::kTvTable)},
    // repeated .ommx.v1.Constraint constraints = 4 [json_name = "constraints"];
    {PROTOBUF_FIELD_OFFSET(Instance, _impl_.constraints_), -1, 3,
    (0 | ::_fl::kFcRepeated | ::_fl::kMessage | ::_fl::kTvTable)},
    // .ommx.v1.Instance.Sense sense = 5 [json_name = "sense"];
    {PROTOBUF_FIELD_OFFSET(Instance, _impl_.sense_), -1, 0,
    (0 | ::_fl::kFcSingular | ::_fl::kOpenEnum)},
  }}, {{
    {::_pbi::TcParser::GetTable<::ommx::v1::Instance_Description>()},
    {::_pbi::TcParser::GetTable<::ommx::v1::DecisionVariable>()},
    {::_pbi::TcParser::GetTable<::ommx::v1::Function>()},
    {::_pbi::TcParser::GetTable<::ommx::v1::Constraint>()},
  }}, {{
  }},
};

::uint8_t* Instance::_InternalSerialize(
    ::uint8_t* target,
    ::google::protobuf::io::EpsCopyOutputStream* stream) const {
  // @@protoc_insertion_point(serialize_to_array_start:ommx.v1.Instance)
  ::uint32_t cached_has_bits = 0;
  (void)cached_has_bits;

  cached_has_bits = _impl_._has_bits_[0];
  // .ommx.v1.Instance.Description description = 1 [json_name = "description"];
  if (cached_has_bits & 0x00000001u) {
    target = ::google::protobuf::internal::WireFormatLite::InternalWriteMessage(
        1, *_impl_.description_, _impl_.description_->GetCachedSize(), target, stream);
  }

  // repeated .ommx.v1.DecisionVariable decision_variables = 2 [json_name = "decisionVariables"];
  for (unsigned i = 0, n = static_cast<unsigned>(
                           this->_internal_decision_variables_size());
       i < n; i++) {
    const auto& repfield = this->_internal_decision_variables().Get(i);
    target =
        ::google::protobuf::internal::WireFormatLite::InternalWriteMessage(
            2, repfield, repfield.GetCachedSize(),
            target, stream);
  }

  // .ommx.v1.Function objective = 3 [json_name = "objective"];
  if (cached_has_bits & 0x00000002u) {
    target = ::google::protobuf::internal::WireFormatLite::InternalWriteMessage(
        3, *_impl_.objective_, _impl_.objective_->GetCachedSize(), target, stream);
  }

  // repeated .ommx.v1.Constraint constraints = 4 [json_name = "constraints"];
  for (unsigned i = 0, n = static_cast<unsigned>(
                           this->_internal_constraints_size());
       i < n; i++) {
    const auto& repfield = this->_internal_constraints().Get(i);
    target =
        ::google::protobuf::internal::WireFormatLite::InternalWriteMessage(
            4, repfield, repfield.GetCachedSize(),
            target, stream);
  }

  // .ommx.v1.Instance.Sense sense = 5 [json_name = "sense"];
  if (this->_internal_sense() != 0) {
    target = stream->EnsureSpace(target);
    target = ::_pbi::WireFormatLite::WriteEnumToArray(
        5, this->_internal_sense(), target);
  }

  if (PROTOBUF_PREDICT_FALSE(_internal_metadata_.have_unknown_fields())) {
    target =
        ::_pbi::WireFormat::InternalSerializeUnknownFieldsToArray(
            _internal_metadata_.unknown_fields<::google::protobuf::UnknownFieldSet>(::google::protobuf::UnknownFieldSet::default_instance), target, stream);
  }
  // @@protoc_insertion_point(serialize_to_array_end:ommx.v1.Instance)
  return target;
}

::size_t Instance::ByteSizeLong() const {
// @@protoc_insertion_point(message_byte_size_start:ommx.v1.Instance)
  ::size_t total_size = 0;

  ::uint32_t cached_has_bits = 0;
  // Prevent compiler warnings about cached_has_bits being unused
  (void) cached_has_bits;

  // repeated .ommx.v1.DecisionVariable decision_variables = 2 [json_name = "decisionVariables"];
  total_size += 1UL * this->_internal_decision_variables_size();
  for (const auto& msg : this->_internal_decision_variables()) {
    total_size += ::google::protobuf::internal::WireFormatLite::MessageSize(msg);
  }
  // repeated .ommx.v1.Constraint constraints = 4 [json_name = "constraints"];
  total_size += 1UL * this->_internal_constraints_size();
  for (const auto& msg : this->_internal_constraints()) {
    total_size += ::google::protobuf::internal::WireFormatLite::MessageSize(msg);
  }
  cached_has_bits = _impl_._has_bits_[0];
  if (cached_has_bits & 0x00000003u) {
    // .ommx.v1.Instance.Description description = 1 [json_name = "description"];
    if (cached_has_bits & 0x00000001u) {
      total_size +=
          1 + ::google::protobuf::internal::WireFormatLite::MessageSize(*_impl_.description_);
    }

    // .ommx.v1.Function objective = 3 [json_name = "objective"];
    if (cached_has_bits & 0x00000002u) {
      total_size +=
          1 + ::google::protobuf::internal::WireFormatLite::MessageSize(*_impl_.objective_);
    }

  }
  // .ommx.v1.Instance.Sense sense = 5 [json_name = "sense"];
  if (this->_internal_sense() != 0) {
    total_size += 1 +
                  ::_pbi::WireFormatLite::EnumSize(this->_internal_sense());
  }

  return MaybeComputeUnknownFieldsSize(total_size, &_impl_._cached_size_);
}


void Instance::MergeImpl(::google::protobuf::MessageLite& to_msg, const ::google::protobuf::MessageLite& from_msg) {
  auto* const _this = static_cast<Instance*>(&to_msg);
  auto& from = static_cast<const Instance&>(from_msg);
  ::google::protobuf::Arena* arena = _this->GetArena();
  // @@protoc_insertion_point(class_specific_merge_from_start:ommx.v1.Instance)
  ABSL_DCHECK_NE(&from, _this);
  ::uint32_t cached_has_bits = 0;
  (void) cached_has_bits;

  _this->_internal_mutable_decision_variables()->MergeFrom(
      from._internal_decision_variables());
  _this->_internal_mutable_constraints()->MergeFrom(
      from._internal_constraints());
  cached_has_bits = from._impl_._has_bits_[0];
  if (cached_has_bits & 0x00000003u) {
    if (cached_has_bits & 0x00000001u) {
      ABSL_DCHECK(from._impl_.description_ != nullptr);
      if (_this->_impl_.description_ == nullptr) {
        _this->_impl_.description_ =
            ::google::protobuf::Message::CopyConstruct<::ommx::v1::Instance_Description>(arena, *from._impl_.description_);
      } else {
        _this->_impl_.description_->MergeFrom(*from._impl_.description_);
      }
    }
    if (cached_has_bits & 0x00000002u) {
      ABSL_DCHECK(from._impl_.objective_ != nullptr);
      if (_this->_impl_.objective_ == nullptr) {
        _this->_impl_.objective_ =
            ::google::protobuf::Message::CopyConstruct<::ommx::v1::Function>(arena, *from._impl_.objective_);
      } else {
        _this->_impl_.objective_->MergeFrom(*from._impl_.objective_);
      }
    }
  }
  if (from._internal_sense() != 0) {
    _this->_impl_.sense_ = from._impl_.sense_;
  }
  _this->_impl_._has_bits_[0] |= cached_has_bits;
  _this->_internal_metadata_.MergeFrom<::google::protobuf::UnknownFieldSet>(from._internal_metadata_);
}

void Instance::CopyFrom(const Instance& from) {
// @@protoc_insertion_point(class_specific_copy_from_start:ommx.v1.Instance)
  if (&from == this) return;
  Clear();
  MergeFrom(from);
}

PROTOBUF_NOINLINE bool Instance::IsInitialized() const {
  return true;
}

void Instance::InternalSwap(Instance* PROTOBUF_RESTRICT other) {
  using std::swap;
  _internal_metadata_.InternalSwap(&other->_internal_metadata_);
  swap(_impl_._has_bits_[0], other->_impl_._has_bits_[0]);
  _impl_.decision_variables_.InternalSwap(&other->_impl_.decision_variables_);
  _impl_.constraints_.InternalSwap(&other->_impl_.constraints_);
  ::google::protobuf::internal::memswap<
      PROTOBUF_FIELD_OFFSET(Instance, _impl_.sense_)
      + sizeof(Instance::_impl_.sense_)
      - PROTOBUF_FIELD_OFFSET(Instance, _impl_.description_)>(
          reinterpret_cast<char*>(&_impl_.description_),
          reinterpret_cast<char*>(&other->_impl_.description_));
}

::google::protobuf::Metadata Instance::GetMetadata() const {
  return ::_pbi::AssignDescriptors(&descriptor_table_ommx_2fv1_2finstance_2eproto_getter,
                                   &descriptor_table_ommx_2fv1_2finstance_2eproto_once,
                                   file_level_metadata_ommx_2fv1_2finstance_2eproto[1]);
}
// @@protoc_insertion_point(namespace_scope)
}  // namespace v1
}  // namespace ommx
namespace google {
namespace protobuf {
}  // namespace protobuf
}  // namespace google
// @@protoc_insertion_point(global_scope)
PROTOBUF_ATTRIBUTE_INIT_PRIORITY2
static ::std::false_type _static_init_ PROTOBUF_UNUSED =
    (::_pbi::AddDescriptors(&descriptor_table_ommx_2fv1_2finstance_2eproto),
     ::std::false_type{});
#include "google/protobuf/port_undef.inc"
