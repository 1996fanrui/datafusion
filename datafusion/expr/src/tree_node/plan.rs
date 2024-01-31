// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Tree node implementation for logical plan

use crate::LogicalPlan;

use datafusion_common::tree_node::{
    Transformed, TransformedIterator, TreeNode, TreeNodeRecursion, TreeNodeVisitor,
};
use datafusion_common::{handle_tree_recursion, Result};

impl TreeNode for LogicalPlan {
    fn apply<F: FnMut(&Self) -> Result<TreeNodeRecursion>>(
        &self,
        op: &mut F,
    ) -> Result<TreeNodeRecursion> {
        // Compared to the default implementation, we need to invoke
        // [`Self::apply_subqueries`] before visiting its children
        handle_tree_recursion!(op(self)?);
        self.apply_subqueries(op)?;
        self.apply_children(&mut |node| node.apply(op))
    }

    /// To use, define a struct that implements the trait [`TreeNodeVisitor`] and then invoke
    /// [`LogicalPlan::visit`].
    ///
    /// For example, for a logical plan like:
    ///
    /// ```text
    /// Projection: id
    ///    Filter: state Eq Utf8(\"CO\")\
    ///       CsvScan: employee.csv projection=Some([0, 3])";
    /// ```
    ///
    /// The sequence of visit operations would be:
    /// ```text
    /// visitor.pre_visit(Projection)
    /// visitor.pre_visit(Filter)
    /// visitor.pre_visit(CsvScan)
    /// visitor.post_visit(CsvScan)
    /// visitor.post_visit(Filter)
    /// visitor.post_visit(Projection)
    /// ```
    fn visit<V: TreeNodeVisitor<N = Self>>(
        &self,
        visitor: &mut V,
    ) -> Result<TreeNodeRecursion> {
        // Compared to the default implementation, we need to invoke
        // [`Self::visit_subqueries`] before visiting its children
        handle_tree_recursion!(visitor.pre_visit(self)?);
        self.visit_subqueries(visitor)?;
        handle_tree_recursion!(self.apply_children(&mut |node| node.visit(visitor))?);
        visitor.post_visit(self)
    }

    fn apply_children<F: FnMut(&Self) -> Result<TreeNodeRecursion>>(
        &self,
        op: &mut F,
    ) -> Result<TreeNodeRecursion> {
        for child in self.inputs() {
            handle_tree_recursion!(op(child)?)
        }
        Ok(TreeNodeRecursion::Continue)
    }

    fn map_children<F>(self, f: F) -> Result<Transformed<Self>>
    where
        F: FnMut(Self) -> Result<Transformed<Self>>,
    {
        let old_children = self.inputs();
        let t = old_children
            .iter()
            .map(|c| (*c).clone())
            .map_till_continue_and_collect(f)?;
        // TODO: once we trust `t.transformed` remove additional check
        if old_children
            .into_iter()
            .zip(t.data.iter())
            .any(|(c1, c2)| c1 != c2)
        {
            Ok(Transformed::new(
                self.with_new_exprs(self.expressions(), t.data)?,
                true,
                t.tnr,
            ))
        } else {
            Ok(Transformed::new(self, false, t.tnr))
        }
    }
}
