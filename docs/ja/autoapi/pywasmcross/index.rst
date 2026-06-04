pywasmcross
===========

.. py:module:: pywasmcross

.. autoapi-nested-parse::

   Helper for cross-compiling Python binary extensions.

   Python has never had a proper cross-compilation story. This is a hack, which
   miraculously works, to get around that.
   The gist is we compile the package replacing calls to the compiler and linker
   with wrappers that adjusting include paths and flags as necessary for
   cross-compiling and then pass the command long to emscripten.



Attributes
----------

.. autoapisummary::

   pywasmcross.INVOKED_PATH
   pywasmcross.IS_COMPILER_INVOCATION
   pywasmcross.PYWASMCROSS_ARGS
   pywasmcross.SYMLINKS


Classes
-------

.. autoapisummary::

   pywasmcross.CrossCompileArgs


Functions
---------

.. autoapisummary::

   pywasmcross.calculate_exports
   pywasmcross.calculate_object_exports_nm
   pywasmcross.calculate_object_exports_readobj
   pywasmcross.compiler_main
   pywasmcross.filter_objects
   pywasmcross.get_cmake_compiler_flags
   pywasmcross.get_export_flags
   pywasmcross.handle_command
   pywasmcross.handle_command_generate_args
   pywasmcross.is_link_cmd
   pywasmcross.replay_genargs_handle_argument
   pywasmcross.replay_genargs_handle_dashI
   pywasmcross.replay_genargs_handle_dashl
   pywasmcross.replay_genargs_handle_linker_opts


Module Contents
---------------

.. py:class:: CrossCompileArgs



   Arguments for cross-compiling a package.


   .. py:attribute:: abi
      :type:  str
      :value: ''



   .. py:attribute:: cflags
      :type:  str
      :value: ''



   .. py:attribute:: cxxflags
      :type:  str
      :value: ''



   .. py:attribute:: exports
      :type:  Literal['whole_archive', 'requested', 'pyinit'] | list[str]
      :value: 'pyinit'



   .. py:attribute:: ldflags
      :type:  str
      :value: ''



   .. py:attribute:: pkgname
      :type:  str
      :value: ''



   .. py:attribute:: pythoninclude
      :type:  str
      :value: ''



   .. py:attribute:: target_install_dir
      :type:  str
      :value: ''



.. py:function:: calculate_exports(line: list[str], export_all: bool) -> collections.abc.Iterable[str]

   List out symbols from object files and archive files that are marked as public.
   If ``export_all`` is ``True``, then return all public symbols.
   If not, return only the public symbols that begin with `PyInit`.


.. py:function:: calculate_object_exports_nm(objects: list[str]) -> list[str]

.. py:function:: calculate_object_exports_readobj(objects: list[str]) -> list[str] | None

.. py:function:: compiler_main()

.. py:function:: filter_objects(line: list[str]) -> list[str]

   Collect up all the object files and archive files being linked.


.. py:function:: get_cmake_compiler_flags() -> list[str]

   Generate cmake compiler flags.
   emcmake will set these values to emcc, em++, ...
   but we need to set them to cc, c++, in order to make them pass to pywasmcross.
   Returns
   -------
   The commandline flags to pass to cmake.


.. py:function:: get_export_flags(line: list[str], exports: Literal['whole_archive', 'requested', 'pyinit'] | list[str]) -> collections.abc.Iterator[str]

   If "whole_archive" was requested, no action is needed. Otherwise, add
   `-sSIDE_MODULE=2` and the appropriate export list.


.. py:function:: handle_command(line: list[str], build_args: CrossCompileArgs) -> int

   Handle a compilation command. Exit with an appropriate exit code when done.

   Parameters
   ----------
   line : iterable
      an iterable with the compilation arguments
   build_args : BuildArgs
      a container with additional compilation options


.. py:function:: handle_command_generate_args(line: list[str], build_args: CrossCompileArgs) -> list[str]

   A helper command for `handle_command` that generates the new arguments for
   the compilation.

   Unlike `handle_command` this avoids I/O: it doesn't sys.exit, it doesn't run
   subprocesses, it doesn't create any files, and it doesn't write to stdout.

   Parameters
   ----------
   line The original compilation command as a list e.g., ["gcc", "-c",
       "input.c", "-o", "output.c"]

   build_args The arguments that pywasmcross was invoked with

   Returns
   -------
       An updated argument list suitable for use with emscripten.

   Examples
   --------

   >>> from collections import namedtuple
   >>> Args = namedtuple('args', ['cflags', 'cxxflags', 'ldflags', 'target_install_dir'])
   >>> args = Args(cflags='', cxxflags='', ldflags='', target_install_dir='')
   >>> handle_command_generate_args(['gcc', 'test.c'], args)
   ['emcc', 'test.c', '-Werror=implicit-function-declaration', '-Werror=mismatched-parameter-types', '-Werror=return-type']


.. py:function:: is_link_cmd(line: list[str]) -> bool

   Check if the command is a linker invocation.


.. py:function:: replay_genargs_handle_argument(arg: str) -> str | None

   Figure out how to replace a general argument.

   Parameters
   ----------
   arg
       The argument we are replacing. Must not start with `-I` or `-l`.

   Returns
   -------
       The new argument, or None to delete the argument.


.. py:function:: replay_genargs_handle_dashI(arg: str, target_install_dir: str) -> str | None

   Figure out how to replace a `-Iincludepath` argument.

   Parameters
   ----------
   arg
       The argument we are replacing. Must start with `-I`.

   target_install_dir
       The target_install_dir argument.

   Returns
   -------
       The new argument, or None to delete the argument.


.. py:function:: replay_genargs_handle_dashl(arg: str, used_libs: set[str], abi: str) -> str | None

   Figure out how to replace a `-lsomelib` argument.

   Parameters
   ----------
   arg
       The argument we are replacing. Must start with `-l`.

   used_libs
       The libraries we've used so far in this command. emcc fails out if `-lsomelib`
       occurs twice, so we have to track this.

   Returns
   -------
       The new argument, or None to delete the argument.


.. py:function:: replay_genargs_handle_linker_opts(arg: str) -> str | None

   ignore some link flags
   it should not check if `arg == "-Wl,-xxx"` and ignore directly here,
   because arg may be something like "-Wl,-xxx,-yyy" where we only want
   to ignore "-xxx" but not "-yyy".


.. py:data:: INVOKED_PATH

.. py:data:: IS_COMPILER_INVOCATION

.. py:data:: PYWASMCROSS_ARGS

.. py:data:: SYMLINKS

