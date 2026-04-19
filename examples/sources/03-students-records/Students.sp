module Students
  ( topScorersByGrade )
  where

-- A student's score row. Uses 09's `type T = τ` transparent
-- alias form (09 §Type aliases, 09 OQ 2 closure).
type Student = { name : String, grade : Int, score : Int }

-- Returns the highest-scoring student per grade, as a pair list.
-- (Shows multi-step pattern matching and record-field access.)
topScorersByGrade : List Student -> List { grade : Int, top : Student }
topScorersByGrade students =
  let grades = uniqueGrades students in
  map (\g -> { grade = g, top = bestIn g students }) grades

-- Extract the distinct grades in the list.
uniqueGrades : List Student -> List Int
uniqueGrades = foldr addGradeIfAbsent []

addGradeIfAbsent : Student -> List Int -> List Int
addGradeIfAbsent s gs =
  if member s.grade gs
    then gs
    else Cons s.grade gs

-- Best scorer inside a single grade. The list is assumed non-empty
-- for grades returned by `uniqueGrades`.
bestIn : Int -> List Student -> Student
bestIn g students =
  let inGrade = filter (\s -> s.grade == g) students in
  foldr1 pickBetter inGrade

pickBetter : Student -> Student -> Student
pickBetter a b =
  if a.score >= b.score then a else b

-- Simple membership.
member : Int -> List Int -> Bool
member _ []       = False
member x (y::ys)  = if x == y then True else member x ys

-- foldr1 is not in the minimum prelude of 09; it's defined here for
-- clarity. Sapphire users who want it in their own prelude can
-- re-export from a utility module.
foldr1 : (a -> a -> a) -> List a -> a
foldr1 f (x::xs) = foldr f x xs
-- foldr1 _ [] is not reachable for the cases this module produces.
