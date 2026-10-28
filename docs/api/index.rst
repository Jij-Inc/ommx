OMMX Reference
==============

Use this index to find SDK APIs, data-format specifications, and integration
contracts. Python and Rust SDK versions are managed independently; Message and
Artifact format versions are separate from both SDK package versions.

.. contents::
   :local:
   :depth: 1

Python SDK API
--------------

The API below is generated from the same development revision as this Book.
It can include features absent from the Book's public ``3.0.0b7`` examples.
For an implementation targeting that release, use the
`public-b7 SDK and Adapter sources <https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python>`_.
When using development features, match the SDK, Adapter, and Reference revision.

Exact operation contracts include :py:meth:`ommx.Instance.prepare` for
Preparation order, conditions, errors, and mutation, and
:py:meth:`ommx.Instance.convert_sos1_to_constraints` for SOS1 Big-M formulas
and applicability.

Core
~~~~

.. toctree::
   :maxdepth: 3

   ommx
   ommx.artifact
   ommx.experiment

Utilities
~~~~~~~~~

.. toctree::
   :maxdepth: 1

   ../autoapi/ommx/adapter/index
   ../autoapi/ommx/cli/index
   ../autoapi/ommx/dataset/index
   ../autoapi/ommx/display/index
   ../autoapi/ommx/testing/index
   ../autoapi/ommx/mps/index
   ../autoapi/ommx/qplib/index
   ../autoapi/ommx/tracing/index

Adapters
~~~~~~~~

.. toctree::
   :maxdepth: 1

   ../autoapi/ommx_python_mip_adapter/index
   ../autoapi/ommx_pyscipopt_adapter/index
   ../autoapi/ommx_highs_adapter/index
   ../autoapi/ommx_openjij_adapter/index


Python indices
~~~~~~~~~~~~~~

* :ref:`genindex`
* :ref:`modindex`

Rust SDK API
------------

Use the `Rust SDK 3.0.0-beta.7 API <https://docs.rs/ommx/3.0.0-beta.7/ommx/>`_
for that public release. Rust readers can go directly from the common problem
and evaluation Guides to Rustdoc without running the Python Tutorials.
For development work, read the source at the commit used by the application.

.. _reference-wire:

Message and Artifact specifications
-----------------------------------

Messages serialize problems, candidates, and results as Protocol Buffers bytes.
Choose the wire schema identified by the payload's Media Type, preserving the
variable IDs referenced by expressions, constraints, and candidate values.
The schema version is independent of the SDK package version.

The following sources are fixed to public Python SDK ``3.0.0b7``:

* `Message schema <https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto/ommx>`_:
  language-independent problem, candidate, and result definitions.
* `Function wire definition <https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/proto/ommx/v1/function.proto>`_:
  the expression definition also used by v2 problem messages.
* `Artifact specification <https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md>`_:
  Manifest, Media Types, archive, and registry interoperability requirements.
* `Experiment implementation and Config contracts <https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/rust/ommx/src/experiment>`_:
  the trial-recording structure stored in Artifacts.

Wire namespaces such as ``ommx.v1`` and ``ommx.v2`` are not Python SDK import
namespaces or package versions. An Artifact's format version also differs from
the versions of its Instance/Solution payloads. Use the specifications above,
rather than Python tutorial code, to implement another language's reader/writer.

Integration contracts
---------------------

* :doc:`Adapter contracts <../autoapi/ommx/adapter/index>`:
  direct input declarations, Preparation, encoding, decoding, and evaluation.
  The `public-b7 Adapter contract source <https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/python/ommx/ommx/adapter.py>`_
  provides the release-matched implementation.
* `Public-b7 AttachmentCodec Protocol <https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/python/ommx/ommx/experiment/attachments.py>`_:
  the provider-owned ``media_type``, ``encode`` and ``decode`` contract for
  attaching external Python objects as bytes and restoring their type.
