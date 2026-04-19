# frozen_string_literal: true
#
# Generated from module Students by sapphire-compiler 0.1.0.
# Targets sapphire-runtime ~> 0.1. Do not edit by hand.
# See docs/build/02-source-and-output-layout.md for the output contract.

require 'sapphire/runtime'
require 'sapphire/prelude'

Sapphire::Runtime.require_version!('~> 0.1')

module Sapphire
  class Students

    def self.topScorersByGrade
      ->(_arg0) { (students = _arg0; (lambda { grades = ((Sapphire::Students.uniqueGrades).call(students)); (((Sapphire::Prelude::MAP).call(->(g) { { grade: (g), top: (((Sapphire::Students.bestIn).call(g)).call(students)) } })).call(grades)) }).call) }
    end

    def self.uniqueGrades
      ((Sapphire::Prelude::FOLDR).call(Sapphire::Students.addGradeIfAbsent)).call([])
    end

    def self.addGradeIfAbsent
      ->(_arg0) { ->(_arg1) { (s = _arg0; gs = _arg1; (((Sapphire::Students.member).call((s)[:grade])).call(gs) ? (gs) : ([(s)[:grade], *gs]))) } }
    end

    def self.bestIn
      ->(_arg0) { ->(_arg1) { (g = _arg0; students = _arg1; (lambda { inGrade = (((Sapphire::Prelude::FILTER).call(->(s) { (((s)[:grade]) == (g)) })).call(students)); (((Sapphire::Students.foldr1).call(Sapphire::Students.pickBetter)).call(inGrade)) }).call) } }
    end

    def self.pickBetter
      ->(_arg0) { ->(_arg1) { (a = _arg0; b = _arg1; ((((a)[:score]) >= ((b)[:score])) ? (a) : (b))) } }
    end

    def self.member
      ->(_arg0) { ->(_arg1) { (case [_arg0, _arg1]; in [_, []]; (Sapphire::Prelude::False); in [x, [y, *ys]]; ((((x) == (y)) ? (Sapphire::Prelude::True) : (((Sapphire::Students.member).call(x)).call(ys)))); else; raise 'non-exhaustive function clauses'; end) } }
    end

    def self.foldr1
      ->(_arg0) { ->(_arg1) { (case [_arg0, _arg1]; in [f, [x, *xs]]; ((((Sapphire::Prelude::FOLDR).call(f)).call(x)).call(xs)); else; raise 'non-exhaustive function clause'; end) } }
    end
  end
end
