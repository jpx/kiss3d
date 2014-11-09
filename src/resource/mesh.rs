//! Data structure of a scene node geometry.

use std::num::Zero;
use sync::{Arc, RWLock};
use gl::types::*;
use na::{Pnt2, Vec3, Pnt3};
use na;
use ncollide::procedural::{TriMesh, TriMesh3, IndexBuffer};
use resource::ShaderAttribute;
use resource::gpu_vector::{GPUVector, DynamicDraw, StaticDraw, ArrayBuffer, ElementArrayBuffer};

#[path = "../error.rs"]
mod error;

/// Aggregation of vertices, indices, normals and texture coordinates.
///
/// It also contains the GPU location of those buffers.
pub struct Mesh {
    coords:  Arc<RWLock<GPUVector<Pnt3<GLfloat>>>>,
    faces:   Arc<RWLock<GPUVector<Vec3<GLuint>>>>,
    normals: Arc<RWLock<GPUVector<Vec3<GLfloat>>>>,
    uvs:     Arc<RWLock<GPUVector<Pnt2<GLfloat>>>>
}

impl Mesh {
    /// Creates a new mesh.
    ///
    /// If the normals and uvs are not given, they are automatically computed.
    pub fn new(coords:       Vec<Pnt3<GLfloat>>,
               faces:        Vec<Vec3<GLuint>>,
               normals:      Option<Vec<Vec3<GLfloat>>>,
               uvs:          Option<Vec<Pnt2<GLfloat>>>,
               dynamic_draw: bool)
               -> Mesh {
        let normals = match normals {
            Some(ns) => ns,
            None     => Mesh::compute_normals_array(coords.as_slice(), faces.as_slice())
        };

        let uvs = match uvs {
            Some(us) => us,
            None     => Vec::from_elem(coords.len(), na::orig())
        };

        let location = if dynamic_draw { DynamicDraw } else { StaticDraw };
        let cs = Arc::new(RWLock::new(GPUVector::new(coords, ArrayBuffer, location)));
        let fs = Arc::new(RWLock::new(GPUVector::new(faces, ElementArrayBuffer, location)));
        let ns = Arc::new(RWLock::new(GPUVector::new(normals, ArrayBuffer, location)));
        let us = Arc::new(RWLock::new(GPUVector::new(uvs, ArrayBuffer, location)));

        Mesh::new_with_gpu_vectors(cs, fs, ns, us)
    }

    /// Creates a new mesh from a mesh descr.
    ///
    /// In the normals and uvs are not given, they are automatically computed.
    pub fn from_trimesh(mesh: TriMesh3<GLfloat>, dynamic_draw: bool) -> Mesh {
        let mut mesh = mesh;

        mesh.unify_index_buffer();

        let TriMesh { coords, normals, uvs, indices } = mesh;
        
        Mesh::new(coords, indices.unwrap_unified(), normals, uvs, dynamic_draw)
    }

    /// Creates a triangle mesh from this mesh.
    pub fn to_trimesh(&self) -> Option<TriMesh3<GLfloat>> {
        let unload_coords  = !self.coords.read().is_on_ram();
        let unload_faces   = !self.faces.read().is_on_ram();
        let unload_normals = !self.normals.read().is_on_ram();
        let unload_uvs     = !self.uvs.read().is_on_ram();

        self.coords.write().load_to_ram();
        self.faces.write().load_to_ram();
        self.normals.write().load_to_ram();
        self.uvs.write().load_to_ram();

        let coords  = self.coords.read().to_owned();
        let faces   = self.faces.read().to_owned();
        let normals = self.normals.read().to_owned();
        let uvs     = self.uvs.read().to_owned();

        if unload_coords {
            self.coords.write().unload_from_ram();
        }
        if unload_faces {
            self.coords.write().unload_from_ram();
        }
        if unload_normals {
            self.coords.write().unload_from_ram();
        }
        if unload_uvs {
            self.coords.write().unload_from_ram();
        }

        if coords.is_none() || faces.is_none() {
            None
        }
        else {
            Some(TriMesh::new(coords.unwrap(), normals, uvs, Some(IndexBuffer::Unified(faces.unwrap()))))
        }
    }

    /// Creates a new mesh. Arguments set to `None` are automatically computed.
    pub fn new_with_gpu_vectors(coords:  Arc<RWLock<GPUVector<Pnt3<GLfloat>>>>,
                                faces:   Arc<RWLock<GPUVector<Vec3<GLuint>>>>,
                                normals: Arc<RWLock<GPUVector<Vec3<GLfloat>>>>,
                                uvs:     Arc<RWLock<GPUVector<Pnt2<GLfloat>>>>)
                                -> Mesh {
        Mesh {
            coords:  coords,
            faces:   faces,
            normals: normals,
            uvs:     uvs
        }
    }

    /// Binds this mesh vertex coordinates buffer to a vertex attribute.
    pub fn bind_coords(&mut self, coords: &mut ShaderAttribute<Pnt3<GLfloat>>) {
        coords.bind(self.coords.write().deref_mut());
    }

    /// Binds this mesh vertex normals buffer to a vertex attribute.
    pub fn bind_normals(&mut self, normals: &mut ShaderAttribute<Vec3<GLfloat>>) {
        normals.bind(self.normals.write().deref_mut());
    }

    /// Binds this mesh vertex uvs buffer to a vertex attribute.
    pub fn bind_uvs(&mut self, uvs: &mut ShaderAttribute<Pnt2<GLfloat>>) {
        uvs.bind(self.uvs.write().deref_mut());
    }

    /// Binds this mesh vertex uvs buffer to a vertex attribute.
    pub fn bind_faces(&mut self) {
        self.faces.write().bind();
    }

    /// Binds this mesh buffers to vertex attributes.
    pub fn bind(&mut self,
                coords:  &mut ShaderAttribute<Pnt3<GLfloat>>,
                normals: &mut ShaderAttribute<Vec3<GLfloat>>,
                uvs:     &mut ShaderAttribute<Pnt2<GLfloat>>) {
        self.bind_coords(coords);
        self.bind_normals(normals);
        self.bind_uvs(uvs);
        self.bind_faces();
    }

    /// Unbind this mesh buffers to vertex attributes.
    pub fn unbind(&self) {
        self.coords.write().unbind();
        self.normals.write().unbind();
        self.uvs.write().unbind();
        self.faces.write().unbind();
    }

    /// Number of points needed to draw this mesh.
    pub fn num_pts(&self) -> uint {
        self.faces.read().len() * 3
    }

    /// Recompute this mesh normals.
    pub fn recompute_normals(&mut self) {
        Mesh::compute_normals(self.coords.read().data().as_ref().unwrap().as_slice(),
                              self.faces.read().data().as_ref().unwrap().as_slice(),
                              self.normals.write().data_mut().as_mut().unwrap());
    }

    /// This mesh faces.
    pub fn faces<'a>(&'a self) -> &'a Arc<RWLock<GPUVector<Vec3<GLuint>>>> {
        &self.faces
    }

    /// This mesh normals.
    pub fn normals<'a>(&'a self) -> &'a Arc<RWLock<GPUVector<Vec3<GLfloat>>>> {
        &self.normals
    }

    /// This mesh vertex coordinates.
    pub fn coords<'a>(&'a self) -> &'a Arc<RWLock<GPUVector<Pnt3<GLfloat>>>> {
        &self.coords
    }

    /// This mesh texture coordinates.
    pub fn uvs<'a>(&'a self) -> &'a Arc<RWLock<GPUVector<Pnt2<GLfloat>>>> {
        &self.uvs
    }

    /// Computes normals from a set of faces.
    pub fn compute_normals_array(coordinates: &[Pnt3<GLfloat>], faces: &[Vec3<GLuint>]) -> Vec<Vec3<GLfloat>> {
        let mut res = Vec::new();
    
        Mesh::compute_normals(coordinates, faces, &mut res);
    
        res
    }
    
    /// Computes normals from a set of faces.
    pub fn compute_normals(coordinates: &[Pnt3<GLfloat>],
                           faces:       &[Vec3<GLuint>],
                           normals:     &mut Vec<Vec3<GLfloat>>) {
        let mut divisor = Vec::from_elem(coordinates.len(), 0f32);
    
        normals.clear();
        normals.grow(coordinates.len(), na::zero());
    
        // Accumulate normals ...
        for f in faces.iter() {
            let edge1  = coordinates[f.y as uint] - coordinates[f.x as uint];
            let edge2  = coordinates[f.z as uint] - coordinates[f.x as uint];
            let cross  = na::cross(&edge1, &edge2);
            let normal;
    
            if !cross.is_zero() {
                normal = na::normalize(&cross)
            }
            else {
                normal = cross
            }
    
            normals[f.x as uint] = normals[f.x as uint] + normal;
            normals[f.y as uint] = normals[f.y as uint] + normal;
            normals[f.z as uint] = normals[f.z as uint] + normal;
    
            divisor[f.x as uint] = divisor[f.x as uint] + 1.0;
            divisor[f.y as uint] = divisor[f.y as uint] + 1.0;
            divisor[f.z as uint] = divisor[f.z as uint] + 1.0;
        }
    
        // ... and compute the mean
        for (n, divisor) in normals.iter_mut().zip(divisor.iter()) {
            *n = *n / *divisor
        }
    }
}
