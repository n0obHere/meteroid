use cornucopia_async::Params;
use meteroid_repository as db;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::{api::services::utils::uuid_gen, db::DbService};

use common_grpc::middleware::server::auth::RequestExt;
use meteroid_grpc::meteroid::api::productfamilies::v1::{
    product_families_service_server::ProductFamiliesService, CreateProductFamilyRequest,
    CreateProductFamilyResponse, GetProductFamilyByExternalIdRequest,
    GetProductFamilyByExternalIdResponse, ListProductFamiliesRequest, ListProductFamiliesResponse,
};

use super::mapping;

#[tonic::async_trait]
impl ProductFamiliesService for DbService {
    #[tracing::instrument(skip_all)]
    async fn list_product_families(
        &self,
        request: Request<ListProductFamiliesRequest>,
    ) -> Result<Response<ListProductFamiliesResponse>, Status> {
        let connection = self.get_connection().await?;

        let families = db::products::list_product_families()
            .bind(&connection, &request.tenant()?)
            .all()
            .await
            .map_err(|e| {
                Status::internal("Unable to list product families")
                    .set_source(Arc::new(e))
                    .clone()
            })?;

        let result = families
            .into_iter()
            .map(mapping::product_family::db_to_server)
            .collect();

        Ok(Response::new(ListProductFamiliesResponse {
            product_families: result,
        }))
    }

    #[tracing::instrument(skip_all)]
    async fn create_product_family(
        &self,
        request: Request<CreateProductFamilyRequest>,
    ) -> Result<Response<CreateProductFamilyResponse>, Status> {
        let tenant_id = request.tenant()?;
        let req = request.into_inner();
        let client = self.pool.get().await.unwrap();

        let params = db::products::CreateProductFamilyParams {
            id: uuid_gen::v7(),
            name: req.name,
            external_id: req.external_id,
            tenant_id,
        };

        let product_family = db::products::create_product_family()
            .params(&client, &params)
            .one()
            .await
            .map_err(|e| {
                Status::internal("Unable to create product family")
                    .set_source(Arc::new(e))
                    .clone()
            })?;

        let rs = mapping::product_family::db_to_server(product_family);
        Ok(Response::new(CreateProductFamilyResponse {
            product_family: Some(rs),
        }))
    }

    #[tracing::instrument(skip_all)]
    async fn get_product_family_by_external_id(
        &self,
        request: Request<GetProductFamilyByExternalIdRequest>,
    ) -> Result<Response<GetProductFamilyByExternalIdResponse>, Status> {
        let tenant_id = request.tenant()?;
        let req = request.into_inner();
        let client = self.pool.get().await.unwrap();

        let params = db::products::GetProductFamilyByExternalIdParams {
            external_id: req.external_id,
            tenant_id,
        };

        let product_family = db::products::get_product_family_by_external_id()
            .params(&client, &params)
            .one()
            .await
            .map_err(|e| {
                Status::internal("Unable to get product family by api name")
                    .set_source(Arc::new(e))
                    .clone()
            })?;

        let rs = mapping::product_family::db_to_server(product_family);
        Ok(Response::new(GetProductFamilyByExternalIdResponse {
            product_family: Some(rs),
        }))
    }
}
