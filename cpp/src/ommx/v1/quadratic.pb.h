// Generated by the protocol buffer compiler.  DO NOT EDIT!
// source: ommx/v1/quadratic.proto
// Protobuf C++ Version: 5.26.1

#ifndef GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2fquadratic_2eproto_2epb_2eh
#define GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2fquadratic_2eproto_2epb_2eh

#include <limits>
#include <string>
#include <type_traits>
#include <utility>

#include "google/protobuf/port_def.inc"
#if PROTOBUF_VERSION != 5026001
#error "Protobuf C++ gencode is built with an incompatible version of"
#error "Protobuf C++ headers/runtime. See"
#error "https://protobuf.dev/support/cross-version-runtime-guarantee/#cpp"
#endif
#include "google/protobuf/port_undef.inc"
#include "google/protobuf/io/coded_stream.h"
#include "google/protobuf/arena.h"
#include "google/protobuf/arenastring.h"
#include "google/protobuf/generated_message_tctable_decl.h"
#include "google/protobuf/generated_message_util.h"
#include "google/protobuf/metadata_lite.h"
#include "google/protobuf/generated_message_reflection.h"
#include "google/protobuf/message.h"
#include "google/protobuf/repeated_field.h"  // IWYU pragma: export
#include "google/protobuf/extension_set.h"  // IWYU pragma: export
#include "google/protobuf/unknown_field_set.h"
#include "ommx/v1/linear.pb.h"
// @@protoc_insertion_point(includes)

// Must be included last.
#include "google/protobuf/port_def.inc"

#define PROTOBUF_INTERNAL_EXPORT_ommx_2fv1_2fquadratic_2eproto

namespace google {
namespace protobuf {
namespace internal {
class AnyMetadata;
}  // namespace internal
}  // namespace protobuf
}  // namespace google

// Internal implementation detail -- do not use these members.
struct TableStruct_ommx_2fv1_2fquadratic_2eproto {
  static const ::uint32_t offsets[];
};
extern const ::google::protobuf::internal::DescriptorTable
    descriptor_table_ommx_2fv1_2fquadratic_2eproto;
namespace ommx {
namespace v1 {
class Quadratic;
struct QuadraticDefaultTypeInternal;
extern QuadraticDefaultTypeInternal _Quadratic_default_instance_;
}  // namespace v1
}  // namespace ommx
namespace google {
namespace protobuf {
}  // namespace protobuf
}  // namespace google

namespace ommx {
namespace v1 {

// ===================================================================


// -------------------------------------------------------------------

class Quadratic final : public ::google::protobuf::Message
/* @@protoc_insertion_point(class_definition:ommx.v1.Quadratic) */ {
 public:
  inline Quadratic() : Quadratic(nullptr) {}
  ~Quadratic() override;
  template <typename = void>
  explicit PROTOBUF_CONSTEXPR Quadratic(
      ::google::protobuf::internal::ConstantInitialized);

  inline Quadratic(const Quadratic& from) : Quadratic(nullptr, from) {}
  inline Quadratic(Quadratic&& from) noexcept
      : Quadratic(nullptr, std::move(from)) {}
  inline Quadratic& operator=(const Quadratic& from) {
    CopyFrom(from);
    return *this;
  }
  inline Quadratic& operator=(Quadratic&& from) noexcept {
    if (this == &from) return *this;
    if (GetArena() == from.GetArena()
#ifdef PROTOBUF_FORCE_COPY_IN_MOVE
        && GetArena() != nullptr
#endif  // !PROTOBUF_FORCE_COPY_IN_MOVE
    ) {
      InternalSwap(&from);
    } else {
      CopyFrom(from);
    }
    return *this;
  }

  inline const ::google::protobuf::UnknownFieldSet& unknown_fields() const
      ABSL_ATTRIBUTE_LIFETIME_BOUND {
    return _internal_metadata_.unknown_fields<::google::protobuf::UnknownFieldSet>(::google::protobuf::UnknownFieldSet::default_instance);
  }
  inline ::google::protobuf::UnknownFieldSet* mutable_unknown_fields()
      ABSL_ATTRIBUTE_LIFETIME_BOUND {
    return _internal_metadata_.mutable_unknown_fields<::google::protobuf::UnknownFieldSet>();
  }

  static const ::google::protobuf::Descriptor* descriptor() {
    return GetDescriptor();
  }
  static const ::google::protobuf::Descriptor* GetDescriptor() {
    return default_instance().GetMetadata().descriptor;
  }
  static const ::google::protobuf::Reflection* GetReflection() {
    return default_instance().GetMetadata().reflection;
  }
  static const Quadratic& default_instance() {
    return *internal_default_instance();
  }
  static inline const Quadratic* internal_default_instance() {
    return reinterpret_cast<const Quadratic*>(
        &_Quadratic_default_instance_);
  }
  static constexpr int kIndexInFileMessages = 0;
  friend void swap(Quadratic& a, Quadratic& b) { a.Swap(&b); }
  inline void Swap(Quadratic* other) {
    if (other == this) return;
#ifdef PROTOBUF_FORCE_COPY_IN_SWAP
    if (GetArena() != nullptr && GetArena() == other->GetArena()) {
#else   // PROTOBUF_FORCE_COPY_IN_SWAP
    if (GetArena() == other->GetArena()) {
#endif  // !PROTOBUF_FORCE_COPY_IN_SWAP
      InternalSwap(other);
    } else {
      ::google::protobuf::internal::GenericSwap(this, other);
    }
  }
  void UnsafeArenaSwap(Quadratic* other) {
    if (other == this) return;
    ABSL_DCHECK(GetArena() == other->GetArena());
    InternalSwap(other);
  }

  // implements Message ----------------------------------------------

  Quadratic* New(::google::protobuf::Arena* arena = nullptr) const final {
    return ::google::protobuf::Message::DefaultConstruct<Quadratic>(arena);
  }
  using ::google::protobuf::Message::CopyFrom;
  void CopyFrom(const Quadratic& from);
  using ::google::protobuf::Message::MergeFrom;
  void MergeFrom(const Quadratic& from) { Quadratic::MergeImpl(*this, from); }

  private:
  static void MergeImpl(
      ::google::protobuf::MessageLite& to_msg,
      const ::google::protobuf::MessageLite& from_msg);

  public:
  ABSL_ATTRIBUTE_REINITIALIZES void Clear() final;
  bool IsInitialized() const final;

  ::size_t ByteSizeLong() const final;
  const char* _InternalParse(const char* ptr, ::google::protobuf::internal::ParseContext* ctx) final;
  ::uint8_t* _InternalSerialize(
      ::uint8_t* target,
      ::google::protobuf::io::EpsCopyOutputStream* stream) const final;
  int GetCachedSize() const { return _impl_._cached_size_.Get(); }

  private:
  void SharedCtor(::google::protobuf::Arena* arena);
  void SharedDtor();
  void InternalSwap(Quadratic* other);
 private:
  friend class ::google::protobuf::internal::AnyMetadata;
  static ::absl::string_view FullMessageName() { return "ommx.v1.Quadratic"; }

 protected:
  explicit Quadratic(::google::protobuf::Arena* arena);
  Quadratic(::google::protobuf::Arena* arena, const Quadratic& from);
  Quadratic(::google::protobuf::Arena* arena, Quadratic&& from) noexcept
      : Quadratic(arena) {
    *this = ::std::move(from);
  }
  const ::google::protobuf::MessageLite::ClassData* GetClassData()
      const final;

 public:
  ::google::protobuf::Metadata GetMetadata() const final;
  // nested types ----------------------------------------------------

  // accessors -------------------------------------------------------
  enum : int {
    kRowsFieldNumber = 1,
    kColumnsFieldNumber = 2,
    kValuesFieldNumber = 3,
    kLinearFieldNumber = 4,
  };
  // repeated uint64 rows = 1 [json_name = "rows"];
  int rows_size() const;
  private:
  int _internal_rows_size() const;

  public:
  void clear_rows() ;
  ::uint64_t rows(int index) const;
  void set_rows(int index, ::uint64_t value);
  void add_rows(::uint64_t value);
  const ::google::protobuf::RepeatedField<::uint64_t>& rows() const;
  ::google::protobuf::RepeatedField<::uint64_t>* mutable_rows();

  private:
  const ::google::protobuf::RepeatedField<::uint64_t>& _internal_rows() const;
  ::google::protobuf::RepeatedField<::uint64_t>* _internal_mutable_rows();

  public:
  // repeated uint64 columns = 2 [json_name = "columns"];
  int columns_size() const;
  private:
  int _internal_columns_size() const;

  public:
  void clear_columns() ;
  ::uint64_t columns(int index) const;
  void set_columns(int index, ::uint64_t value);
  void add_columns(::uint64_t value);
  const ::google::protobuf::RepeatedField<::uint64_t>& columns() const;
  ::google::protobuf::RepeatedField<::uint64_t>* mutable_columns();

  private:
  const ::google::protobuf::RepeatedField<::uint64_t>& _internal_columns() const;
  ::google::protobuf::RepeatedField<::uint64_t>* _internal_mutable_columns();

  public:
  // repeated double values = 3 [json_name = "values"];
  int values_size() const;
  private:
  int _internal_values_size() const;

  public:
  void clear_values() ;
  double values(int index) const;
  void set_values(int index, double value);
  void add_values(double value);
  const ::google::protobuf::RepeatedField<double>& values() const;
  ::google::protobuf::RepeatedField<double>* mutable_values();

  private:
  const ::google::protobuf::RepeatedField<double>& _internal_values() const;
  ::google::protobuf::RepeatedField<double>* _internal_mutable_values();

  public:
  // optional .ommx.v1.Linear linear = 4 [json_name = "linear"];
  bool has_linear() const;
  void clear_linear() ;
  const ::ommx::v1::Linear& linear() const;
  PROTOBUF_NODISCARD ::ommx::v1::Linear* release_linear();
  ::ommx::v1::Linear* mutable_linear();
  void set_allocated_linear(::ommx::v1::Linear* value);
  void unsafe_arena_set_allocated_linear(::ommx::v1::Linear* value);
  ::ommx::v1::Linear* unsafe_arena_release_linear();

  private:
  const ::ommx::v1::Linear& _internal_linear() const;
  ::ommx::v1::Linear* _internal_mutable_linear();

  public:
  // @@protoc_insertion_point(class_scope:ommx.v1.Quadratic)
 private:
  class _Internal;
  friend class ::google::protobuf::internal::TcParser;
  static const ::google::protobuf::internal::TcParseTable<
      2, 4, 1,
      0, 2>
      _table_;
  friend class ::google::protobuf::MessageLite;
  friend class ::google::protobuf::Arena;
  template <typename T>
  friend class ::google::protobuf::Arena::InternalHelper;
  using InternalArenaConstructable_ = void;
  using DestructorSkippable_ = void;
  struct Impl_ {
    inline explicit constexpr Impl_(
        ::google::protobuf::internal::ConstantInitialized) noexcept;
    inline explicit Impl_(::google::protobuf::internal::InternalVisibility visibility,
                          ::google::protobuf::Arena* arena);
    inline explicit Impl_(::google::protobuf::internal::InternalVisibility visibility,
                          ::google::protobuf::Arena* arena, const Impl_& from);
    ::google::protobuf::internal::HasBits<1> _has_bits_;
    mutable ::google::protobuf::internal::CachedSize _cached_size_;
    ::google::protobuf::RepeatedField<::uint64_t> rows_;
    mutable ::google::protobuf::internal::CachedSize _rows_cached_byte_size_;
    ::google::protobuf::RepeatedField<::uint64_t> columns_;
    mutable ::google::protobuf::internal::CachedSize _columns_cached_byte_size_;
    ::google::protobuf::RepeatedField<double> values_;
    ::ommx::v1::Linear* linear_;
    PROTOBUF_TSAN_DECLARE_MEMBER
  };
  union { Impl_ _impl_; };
  friend struct ::TableStruct_ommx_2fv1_2fquadratic_2eproto;
};

// ===================================================================




// ===================================================================


#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wstrict-aliasing"
#endif  // __GNUC__
// -------------------------------------------------------------------

// Quadratic

// repeated uint64 rows = 1 [json_name = "rows"];
inline int Quadratic::_internal_rows_size() const {
  return _internal_rows().size();
}
inline int Quadratic::rows_size() const {
  return _internal_rows_size();
}
inline void Quadratic::clear_rows() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _impl_.rows_.Clear();
}
inline ::uint64_t Quadratic::rows(int index) const {
  // @@protoc_insertion_point(field_get:ommx.v1.Quadratic.rows)
  return _internal_rows().Get(index);
}
inline void Quadratic::set_rows(int index, ::uint64_t value) {
  _internal_mutable_rows()->Set(index, value);
  // @@protoc_insertion_point(field_set:ommx.v1.Quadratic.rows)
}
inline void Quadratic::add_rows(::uint64_t value) {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _internal_mutable_rows()->Add(value);
  // @@protoc_insertion_point(field_add:ommx.v1.Quadratic.rows)
}
inline const ::google::protobuf::RepeatedField<::uint64_t>& Quadratic::rows() const
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_list:ommx.v1.Quadratic.rows)
  return _internal_rows();
}
inline ::google::protobuf::RepeatedField<::uint64_t>* Quadratic::mutable_rows()
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_mutable_list:ommx.v1.Quadratic.rows)
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  return _internal_mutable_rows();
}
inline const ::google::protobuf::RepeatedField<::uint64_t>&
Quadratic::_internal_rows() const {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return _impl_.rows_;
}
inline ::google::protobuf::RepeatedField<::uint64_t>* Quadratic::_internal_mutable_rows() {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return &_impl_.rows_;
}

// repeated uint64 columns = 2 [json_name = "columns"];
inline int Quadratic::_internal_columns_size() const {
  return _internal_columns().size();
}
inline int Quadratic::columns_size() const {
  return _internal_columns_size();
}
inline void Quadratic::clear_columns() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _impl_.columns_.Clear();
}
inline ::uint64_t Quadratic::columns(int index) const {
  // @@protoc_insertion_point(field_get:ommx.v1.Quadratic.columns)
  return _internal_columns().Get(index);
}
inline void Quadratic::set_columns(int index, ::uint64_t value) {
  _internal_mutable_columns()->Set(index, value);
  // @@protoc_insertion_point(field_set:ommx.v1.Quadratic.columns)
}
inline void Quadratic::add_columns(::uint64_t value) {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _internal_mutable_columns()->Add(value);
  // @@protoc_insertion_point(field_add:ommx.v1.Quadratic.columns)
}
inline const ::google::protobuf::RepeatedField<::uint64_t>& Quadratic::columns() const
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_list:ommx.v1.Quadratic.columns)
  return _internal_columns();
}
inline ::google::protobuf::RepeatedField<::uint64_t>* Quadratic::mutable_columns()
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_mutable_list:ommx.v1.Quadratic.columns)
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  return _internal_mutable_columns();
}
inline const ::google::protobuf::RepeatedField<::uint64_t>&
Quadratic::_internal_columns() const {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return _impl_.columns_;
}
inline ::google::protobuf::RepeatedField<::uint64_t>* Quadratic::_internal_mutable_columns() {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return &_impl_.columns_;
}

// repeated double values = 3 [json_name = "values"];
inline int Quadratic::_internal_values_size() const {
  return _internal_values().size();
}
inline int Quadratic::values_size() const {
  return _internal_values_size();
}
inline void Quadratic::clear_values() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _impl_.values_.Clear();
}
inline double Quadratic::values(int index) const {
  // @@protoc_insertion_point(field_get:ommx.v1.Quadratic.values)
  return _internal_values().Get(index);
}
inline void Quadratic::set_values(int index, double value) {
  _internal_mutable_values()->Set(index, value);
  // @@protoc_insertion_point(field_set:ommx.v1.Quadratic.values)
}
inline void Quadratic::add_values(double value) {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  _internal_mutable_values()->Add(value);
  // @@protoc_insertion_point(field_add:ommx.v1.Quadratic.values)
}
inline const ::google::protobuf::RepeatedField<double>& Quadratic::values() const
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_list:ommx.v1.Quadratic.values)
  return _internal_values();
}
inline ::google::protobuf::RepeatedField<double>* Quadratic::mutable_values()
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_mutable_list:ommx.v1.Quadratic.values)
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  return _internal_mutable_values();
}
inline const ::google::protobuf::RepeatedField<double>&
Quadratic::_internal_values() const {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return _impl_.values_;
}
inline ::google::protobuf::RepeatedField<double>* Quadratic::_internal_mutable_values() {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  return &_impl_.values_;
}

// optional .ommx.v1.Linear linear = 4 [json_name = "linear"];
inline bool Quadratic::has_linear() const {
  bool value = (_impl_._has_bits_[0] & 0x00000001u) != 0;
  PROTOBUF_ASSUME(!value || _impl_.linear_ != nullptr);
  return value;
}
inline const ::ommx::v1::Linear& Quadratic::_internal_linear() const {
  PROTOBUF_TSAN_READ(&_impl_._tsan_detect_race);
  const ::ommx::v1::Linear* p = _impl_.linear_;
  return p != nullptr ? *p : reinterpret_cast<const ::ommx::v1::Linear&>(::ommx::v1::_Linear_default_instance_);
}
inline const ::ommx::v1::Linear& Quadratic::linear() const ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_get:ommx.v1.Quadratic.linear)
  return _internal_linear();
}
inline void Quadratic::unsafe_arena_set_allocated_linear(::ommx::v1::Linear* value) {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  if (GetArena() == nullptr) {
    delete reinterpret_cast<::google::protobuf::MessageLite*>(_impl_.linear_);
  }
  _impl_.linear_ = reinterpret_cast<::ommx::v1::Linear*>(value);
  if (value != nullptr) {
    _impl_._has_bits_[0] |= 0x00000001u;
  } else {
    _impl_._has_bits_[0] &= ~0x00000001u;
  }
  // @@protoc_insertion_point(field_unsafe_arena_set_allocated:ommx.v1.Quadratic.linear)
}
inline ::ommx::v1::Linear* Quadratic::release_linear() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);

  _impl_._has_bits_[0] &= ~0x00000001u;
  ::ommx::v1::Linear* released = _impl_.linear_;
  _impl_.linear_ = nullptr;
#ifdef PROTOBUF_FORCE_COPY_IN_RELEASE
  auto* old = reinterpret_cast<::google::protobuf::MessageLite*>(released);
  released = ::google::protobuf::internal::DuplicateIfNonNull(released);
  if (GetArena() == nullptr) {
    delete old;
  }
#else   // PROTOBUF_FORCE_COPY_IN_RELEASE
  if (GetArena() != nullptr) {
    released = ::google::protobuf::internal::DuplicateIfNonNull(released);
  }
#endif  // !PROTOBUF_FORCE_COPY_IN_RELEASE
  return released;
}
inline ::ommx::v1::Linear* Quadratic::unsafe_arena_release_linear() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  // @@protoc_insertion_point(field_release:ommx.v1.Quadratic.linear)

  _impl_._has_bits_[0] &= ~0x00000001u;
  ::ommx::v1::Linear* temp = _impl_.linear_;
  _impl_.linear_ = nullptr;
  return temp;
}
inline ::ommx::v1::Linear* Quadratic::_internal_mutable_linear() {
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  if (_impl_.linear_ == nullptr) {
    auto* p = ::google::protobuf::Message::DefaultConstruct<::ommx::v1::Linear>(GetArena());
    _impl_.linear_ = reinterpret_cast<::ommx::v1::Linear*>(p);
  }
  return _impl_.linear_;
}
inline ::ommx::v1::Linear* Quadratic::mutable_linear() ABSL_ATTRIBUTE_LIFETIME_BOUND {
  _impl_._has_bits_[0] |= 0x00000001u;
  ::ommx::v1::Linear* _msg = _internal_mutable_linear();
  // @@protoc_insertion_point(field_mutable:ommx.v1.Quadratic.linear)
  return _msg;
}
inline void Quadratic::set_allocated_linear(::ommx::v1::Linear* value) {
  ::google::protobuf::Arena* message_arena = GetArena();
  PROTOBUF_TSAN_WRITE(&_impl_._tsan_detect_race);
  if (message_arena == nullptr) {
    delete reinterpret_cast<::google::protobuf::MessageLite*>(_impl_.linear_);
  }

  if (value != nullptr) {
    ::google::protobuf::Arena* submessage_arena = reinterpret_cast<::google::protobuf::MessageLite*>(value)->GetArena();
    if (message_arena != submessage_arena) {
      value = ::google::protobuf::internal::GetOwnedMessage(message_arena, value, submessage_arena);
    }
    _impl_._has_bits_[0] |= 0x00000001u;
  } else {
    _impl_._has_bits_[0] &= ~0x00000001u;
  }

  _impl_.linear_ = reinterpret_cast<::ommx::v1::Linear*>(value);
  // @@protoc_insertion_point(field_set_allocated:ommx.v1.Quadratic.linear)
}

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif  // __GNUC__

// @@protoc_insertion_point(namespace_scope)
}  // namespace v1
}  // namespace ommx


// @@protoc_insertion_point(global_scope)

#include "google/protobuf/port_undef.inc"

#endif  // GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2fquadratic_2eproto_2epb_2eh