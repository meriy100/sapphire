# frozen_string_literal: true

module Sapphire
  module Runtime
    # Tagged-hash ADT helpers.
    #
    # Per docs/spec/10-ruby-interop.md §ADTs, a Sapphire ADT value
    # `K v1 ... vk` marshals to a Ruby hash
    # `{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }`. This module
    # is the small helper surface the generated Ruby code uses to
    # build and inspect those hashes.
    #
    # Contract reference: docs/build/03-sapphire-runtime.md §ADT
    # helpers.
    #
    # TODO: implement in R2 (see docs/impl/06-implementation-roadmap.md
    # §Track R).
    module ADT
    end
  end
end
