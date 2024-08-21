// Generated by the protocol buffer compiler.  DO NOT EDIT!
// NO CHECKED-IN PROTOBUF GENCODE
// source: ommx/v1/function.proto
// Protobuf C++ Version: 5.27.3

#ifndef GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2ffunction_2eproto_2epb_2eh
#define GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2ffunction_2eproto_2epb_2eh

#include <limits>
#include <string>
#include <type_traits>
#include <utility>

#include "google/protobuf/runtime_version.h"
#if PROTOBUF_VERSION != 5027003
#error "Protobuf C++ gencode is built with an incompatible version of"
#error "Protobuf C++ headers/runtime. See"
#error "https://protobuf.dev/support/cross-version-runtime-guarantee/#cpp"
#endif
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
#include "ommx/v1/polynomial.pb.h"
#include "ommx/v1/quadratic.pb.h"
// @@protoc_insertion_point(includes)

// Must be included last.
#include "google/protobuf/port_def.inc"

#define PROTOBUF_INTERNAL_EXPORT_ommx_2fv1_2ffunction_2eproto

namespace google {
namespace protobuf {
namespace internal {
class AnyMetadata;
}  // namespace internal
}  // namespace protobuf
}  // namespace google

// Internal implementation detail -- do not use these members.
struct TableStruct_ommx_2fv1_2ffunction_2eproto {
  static const ::uint32_t offsets[];
};
extern const ::google::protobuf::internal::DescriptorTable
    descriptor_table_ommx_2fv1_2ffunction_2eproto;
namespace ommx {
namespace v1 {
class Function;
struct FunctionDefaultTypeInternal;
extern FunctionDefaultTypeInternal _Function_default_instance_;
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

class Function final : public ::google::protobuf::Message
/* @@protoc_insertion_point(class_definition:ommx.v1.Function) */ {
 public:
  inline Function() : Function(nullptr) {}
  ~Function() override;
  template <typename = void>
  explicit PROTOBUF_CONSTEXPR Function(
      ::google::protobuf::internal::ConstantInitialized);

  inline Function(const Function& from) : Function(nullptr, from) {}
  inline Function(Function&& from) noexcept
      : Function(nullptr, std::move(from)) {}
  inline Function& operator=(const Function& from) {
    CopyFrom(from);
    return *this;
  }
  inline Function& operator=(Function&& from) noexcept {
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
  static const Function& default_instance() {
    return *internal_default_instance();
  }
  enum FunctionCase {
    kConstant = 1,
    kLinear = 2,
    kQuadratic = 3,
    kPolynomial = 4,
    FUNCTION_NOT_SET = 0,
  };
  static inline const Function* internal_default_instance() {
    return reinterpret_cast<const Function*>(
        &_Function_default_instance_);
  }
  static constexpr int kIndexInFileMessages = 0;
  friend void swap(Function& a, Function& b) { a.Swap(&b); }
  inline void Swap(Function* other) {
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
  void UnsafeArenaSwap(Function* other) {
    if (other == this) return;
    ABSL_DCHECK(GetArena() == other->GetArena());
    InternalSwap(other);
  }

  // implements Message ----------------------------------------------

  Function* New(::google::protobuf::Arena* arena = nullptr) const final {
    return ::google::protobuf::Message::DefaultConstruct<Function>(arena);
  }
  using ::google::protobuf::Message::CopyFrom;
  void CopyFrom(const Function& from);
  using ::google::protobuf::Message::MergeFrom;
  void MergeFrom(const Function& from) { Function::MergeImpl(*this, from); }

  private:
  static void MergeImpl(
      ::google::protobuf::MessageLite& to_msg,
      const ::google::protobuf::MessageLite& from_msg);

  public:
  bool IsInitialized() const {
    return true;
  }
  ABSL_ATTRIBUTE_REINITIALIZES void Clear() final;
  ::size_t ByteSizeLong() const final;
  ::uint8_t* _InternalSerialize(
      ::uint8_t* target,
      ::google::protobuf::io::EpsCopyOutputStream* stream) const final;
  int GetCachedSize() const { return _impl_._cached_size_.Get(); }

  private:
  void SharedCtor(::google::protobuf::Arena* arena);
  void SharedDtor();
  void InternalSwap(Function* other);
 private:
  friend class ::google::protobuf::internal::AnyMetadata;
  static ::absl::string_view FullMessageName() { return "ommx.v1.Function"; }

 protected:
  explicit Function(::google::protobuf::Arena* arena);
  Function(::google::protobuf::Arena* arena, const Function& from);
  Function(::google::protobuf::Arena* arena, Function&& from) noexcept
      : Function(arena) {
    *this = ::std::move(from);
  }
  const ::google::protobuf::Message::ClassData* GetClassData() const final;

 public:
  ::google::protobuf::Metadata GetMetadata() const;
  // nested types ----------------------------------------------------

  // accessors -------------------------------------------------------
  enum : int {
    kConstantFieldNumber = 1,
    kLinearFieldNumber = 2,
    kQuadraticFieldNumber = 3,
    kPolynomialFieldNumber = 4,
  };
  // double constant = 1 [json_name = "constant"];
  bool has_constant() const;
  void clear_constant() ;
  double constant() const;
  void set_constant(double value);

  private:
  double _internal_constant() const;
  void _internal_set_constant(double value);

  public:
  // .ommx.v1.Linear linear = 2 [json_name = "linear"];
  bool has_linear() const;
  private:
  bool _internal_has_linear() const;

  public:
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
  // .ommx.v1.Quadratic quadratic = 3 [json_name = "quadratic"];
  bool has_quadratic() const;
  private:
  bool _internal_has_quadratic() const;

  public:
  void clear_quadratic() ;
  const ::ommx::v1::Quadratic& quadratic() const;
  PROTOBUF_NODISCARD ::ommx::v1::Quadratic* release_quadratic();
  ::ommx::v1::Quadratic* mutable_quadratic();
  void set_allocated_quadratic(::ommx::v1::Quadratic* value);
  void unsafe_arena_set_allocated_quadratic(::ommx::v1::Quadratic* value);
  ::ommx::v1::Quadratic* unsafe_arena_release_quadratic();

  private:
  const ::ommx::v1::Quadratic& _internal_quadratic() const;
  ::ommx::v1::Quadratic* _internal_mutable_quadratic();

  public:
  // .ommx.v1.Polynomial polynomial = 4 [json_name = "polynomial"];
  bool has_polynomial() const;
  private:
  bool _internal_has_polynomial() const;

  public:
  void clear_polynomial() ;
  const ::ommx::v1::Polynomial& polynomial() const;
  PROTOBUF_NODISCARD ::ommx::v1::Polynomial* release_polynomial();
  ::ommx::v1::Polynomial* mutable_polynomial();
  void set_allocated_polynomial(::ommx::v1::Polynomial* value);
  void unsafe_arena_set_allocated_polynomial(::ommx::v1::Polynomial* value);
  ::ommx::v1::Polynomial* unsafe_arena_release_polynomial();

  private:
  const ::ommx::v1::Polynomial& _internal_polynomial() const;
  ::ommx::v1::Polynomial* _internal_mutable_polynomial();

  public:
  void clear_function();
  FunctionCase function_case() const;
  // @@protoc_insertion_point(class_scope:ommx.v1.Function)
 private:
  class _Internal;
  void set_has_constant();
  void set_has_linear();
  void set_has_quadratic();
  void set_has_polynomial();
  inline bool has_function() const;
  inline void clear_has_function();
  friend class ::google::protobuf::internal::TcParser;
  static const ::google::protobuf::internal::TcParseTable<
      0, 4, 3,
      0, 2>
      _table_;

  static constexpr const void* _raw_default_instance_ =
      &_Function_default_instance_;

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
                          ::google::protobuf::Arena* arena, const Impl_& from,
                          const Function& from_msg);
    union FunctionUnion {
      constexpr FunctionUnion() : _constinit_{} {}
      ::google::protobuf::internal::ConstantInitialized _constinit_;
      double constant_;
      ::ommx::v1::Linear* linear_;
      ::ommx::v1::Quadratic* quadratic_;
      ::ommx::v1::Polynomial* polynomial_;
    } function_;
    mutable ::google::protobuf::internal::CachedSize _cached_size_;
    ::uint32_t _oneof_case_[1];
    PROTOBUF_TSAN_DECLARE_MEMBER
  };
  union { Impl_ _impl_; };
  friend struct ::TableStruct_ommx_2fv1_2ffunction_2eproto;
};

// ===================================================================




// ===================================================================


#ifdef __GNUC__
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wstrict-aliasing"
#endif  // __GNUC__
// -------------------------------------------------------------------

// Function

// double constant = 1 [json_name = "constant"];
inline bool Function::has_constant() const {
  return function_case() == kConstant;
}
inline void Function::set_has_constant() {
  _impl_._oneof_case_[0] = kConstant;
}
inline void Function::clear_constant() {
  ::google::protobuf::internal::TSanWrite(&_impl_);
  if (function_case() == kConstant) {
    _impl_.function_.constant_ = 0;
    clear_has_function();
  }
}
inline double Function::constant() const {
  // @@protoc_insertion_point(field_get:ommx.v1.Function.constant)
  return _internal_constant();
}
inline void Function::set_constant(double value) {
  if (function_case() != kConstant) {
    clear_function();
    set_has_constant();
  }
  _impl_.function_.constant_ = value;
  // @@protoc_insertion_point(field_set:ommx.v1.Function.constant)
}
inline double Function::_internal_constant() const {
  if (function_case() == kConstant) {
    return _impl_.function_.constant_;
  }
  return 0;
}

// .ommx.v1.Linear linear = 2 [json_name = "linear"];
inline bool Function::has_linear() const {
  return function_case() == kLinear;
}
inline bool Function::_internal_has_linear() const {
  return function_case() == kLinear;
}
inline void Function::set_has_linear() {
  _impl_._oneof_case_[0] = kLinear;
}
inline ::ommx::v1::Linear* Function::release_linear() {
  // @@protoc_insertion_point(field_release:ommx.v1.Function.linear)
  if (function_case() == kLinear) {
    clear_has_function();
    auto* temp = _impl_.function_.linear_;
    if (GetArena() != nullptr) {
      temp = ::google::protobuf::internal::DuplicateIfNonNull(temp);
    }
    _impl_.function_.linear_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline const ::ommx::v1::Linear& Function::_internal_linear() const {
  return function_case() == kLinear ? *_impl_.function_.linear_ : reinterpret_cast<::ommx::v1::Linear&>(::ommx::v1::_Linear_default_instance_);
}
inline const ::ommx::v1::Linear& Function::linear() const ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_get:ommx.v1.Function.linear)
  return _internal_linear();
}
inline ::ommx::v1::Linear* Function::unsafe_arena_release_linear() {
  // @@protoc_insertion_point(field_unsafe_arena_release:ommx.v1.Function.linear)
  if (function_case() == kLinear) {
    clear_has_function();
    auto* temp = _impl_.function_.linear_;
    _impl_.function_.linear_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline void Function::unsafe_arena_set_allocated_linear(::ommx::v1::Linear* value) {
  // We rely on the oneof clear method to free the earlier contents
  // of this oneof. We can directly use the pointer we're given to
  // set the new value.
  clear_function();
  if (value) {
    set_has_linear();
    _impl_.function_.linear_ = value;
  }
  // @@protoc_insertion_point(field_unsafe_arena_set_allocated:ommx.v1.Function.linear)
}
inline ::ommx::v1::Linear* Function::_internal_mutable_linear() {
  if (function_case() != kLinear) {
    clear_function();
    set_has_linear();
    _impl_.function_.linear_ =
        ::google::protobuf::Message::DefaultConstruct<::ommx::v1::Linear>(GetArena());
  }
  return _impl_.function_.linear_;
}
inline ::ommx::v1::Linear* Function::mutable_linear() ABSL_ATTRIBUTE_LIFETIME_BOUND {
  ::ommx::v1::Linear* _msg = _internal_mutable_linear();
  // @@protoc_insertion_point(field_mutable:ommx.v1.Function.linear)
  return _msg;
}

// .ommx.v1.Quadratic quadratic = 3 [json_name = "quadratic"];
inline bool Function::has_quadratic() const {
  return function_case() == kQuadratic;
}
inline bool Function::_internal_has_quadratic() const {
  return function_case() == kQuadratic;
}
inline void Function::set_has_quadratic() {
  _impl_._oneof_case_[0] = kQuadratic;
}
inline ::ommx::v1::Quadratic* Function::release_quadratic() {
  // @@protoc_insertion_point(field_release:ommx.v1.Function.quadratic)
  if (function_case() == kQuadratic) {
    clear_has_function();
    auto* temp = _impl_.function_.quadratic_;
    if (GetArena() != nullptr) {
      temp = ::google::protobuf::internal::DuplicateIfNonNull(temp);
    }
    _impl_.function_.quadratic_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline const ::ommx::v1::Quadratic& Function::_internal_quadratic() const {
  return function_case() == kQuadratic ? *_impl_.function_.quadratic_ : reinterpret_cast<::ommx::v1::Quadratic&>(::ommx::v1::_Quadratic_default_instance_);
}
inline const ::ommx::v1::Quadratic& Function::quadratic() const ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_get:ommx.v1.Function.quadratic)
  return _internal_quadratic();
}
inline ::ommx::v1::Quadratic* Function::unsafe_arena_release_quadratic() {
  // @@protoc_insertion_point(field_unsafe_arena_release:ommx.v1.Function.quadratic)
  if (function_case() == kQuadratic) {
    clear_has_function();
    auto* temp = _impl_.function_.quadratic_;
    _impl_.function_.quadratic_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline void Function::unsafe_arena_set_allocated_quadratic(::ommx::v1::Quadratic* value) {
  // We rely on the oneof clear method to free the earlier contents
  // of this oneof. We can directly use the pointer we're given to
  // set the new value.
  clear_function();
  if (value) {
    set_has_quadratic();
    _impl_.function_.quadratic_ = value;
  }
  // @@protoc_insertion_point(field_unsafe_arena_set_allocated:ommx.v1.Function.quadratic)
}
inline ::ommx::v1::Quadratic* Function::_internal_mutable_quadratic() {
  if (function_case() != kQuadratic) {
    clear_function();
    set_has_quadratic();
    _impl_.function_.quadratic_ =
        ::google::protobuf::Message::DefaultConstruct<::ommx::v1::Quadratic>(GetArena());
  }
  return _impl_.function_.quadratic_;
}
inline ::ommx::v1::Quadratic* Function::mutable_quadratic() ABSL_ATTRIBUTE_LIFETIME_BOUND {
  ::ommx::v1::Quadratic* _msg = _internal_mutable_quadratic();
  // @@protoc_insertion_point(field_mutable:ommx.v1.Function.quadratic)
  return _msg;
}

// .ommx.v1.Polynomial polynomial = 4 [json_name = "polynomial"];
inline bool Function::has_polynomial() const {
  return function_case() == kPolynomial;
}
inline bool Function::_internal_has_polynomial() const {
  return function_case() == kPolynomial;
}
inline void Function::set_has_polynomial() {
  _impl_._oneof_case_[0] = kPolynomial;
}
inline ::ommx::v1::Polynomial* Function::release_polynomial() {
  // @@protoc_insertion_point(field_release:ommx.v1.Function.polynomial)
  if (function_case() == kPolynomial) {
    clear_has_function();
    auto* temp = _impl_.function_.polynomial_;
    if (GetArena() != nullptr) {
      temp = ::google::protobuf::internal::DuplicateIfNonNull(temp);
    }
    _impl_.function_.polynomial_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline const ::ommx::v1::Polynomial& Function::_internal_polynomial() const {
  return function_case() == kPolynomial ? *_impl_.function_.polynomial_ : reinterpret_cast<::ommx::v1::Polynomial&>(::ommx::v1::_Polynomial_default_instance_);
}
inline const ::ommx::v1::Polynomial& Function::polynomial() const ABSL_ATTRIBUTE_LIFETIME_BOUND {
  // @@protoc_insertion_point(field_get:ommx.v1.Function.polynomial)
  return _internal_polynomial();
}
inline ::ommx::v1::Polynomial* Function::unsafe_arena_release_polynomial() {
  // @@protoc_insertion_point(field_unsafe_arena_release:ommx.v1.Function.polynomial)
  if (function_case() == kPolynomial) {
    clear_has_function();
    auto* temp = _impl_.function_.polynomial_;
    _impl_.function_.polynomial_ = nullptr;
    return temp;
  } else {
    return nullptr;
  }
}
inline void Function::unsafe_arena_set_allocated_polynomial(::ommx::v1::Polynomial* value) {
  // We rely on the oneof clear method to free the earlier contents
  // of this oneof. We can directly use the pointer we're given to
  // set the new value.
  clear_function();
  if (value) {
    set_has_polynomial();
    _impl_.function_.polynomial_ = value;
  }
  // @@protoc_insertion_point(field_unsafe_arena_set_allocated:ommx.v1.Function.polynomial)
}
inline ::ommx::v1::Polynomial* Function::_internal_mutable_polynomial() {
  if (function_case() != kPolynomial) {
    clear_function();
    set_has_polynomial();
    _impl_.function_.polynomial_ =
        ::google::protobuf::Message::DefaultConstruct<::ommx::v1::Polynomial>(GetArena());
  }
  return _impl_.function_.polynomial_;
}
inline ::ommx::v1::Polynomial* Function::mutable_polynomial() ABSL_ATTRIBUTE_LIFETIME_BOUND {
  ::ommx::v1::Polynomial* _msg = _internal_mutable_polynomial();
  // @@protoc_insertion_point(field_mutable:ommx.v1.Function.polynomial)
  return _msg;
}

inline bool Function::has_function() const {
  return function_case() != FUNCTION_NOT_SET;
}
inline void Function::clear_has_function() {
  _impl_._oneof_case_[0] = FUNCTION_NOT_SET;
}
inline Function::FunctionCase Function::function_case() const {
  return Function::FunctionCase(_impl_._oneof_case_[0]);
}
#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif  // __GNUC__

// @@protoc_insertion_point(namespace_scope)
}  // namespace v1
}  // namespace ommx


// @@protoc_insertion_point(global_scope)

#include "google/protobuf/port_undef.inc"

#endif  // GOOGLE_PROTOBUF_INCLUDED_ommx_2fv1_2ffunction_2eproto_2epb_2eh
